package com.musicfrog.infiltrator

import android.app.Activity
import android.app.Application
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.util.Log
import android.view.View
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import org.json.JSONObject
import java.util.WeakHashMap
import java.util.concurrent.atomic.AtomicLong

/**
 * BANDROID-008: the clipboard channel Rust calls back into through JNI. The
 * Activity owns the real `ClipboardManager`; no `Context`/`ClipData` type
 * crosses the shared port boundary.
 */
interface NativeHostBridge {
    /** The current clipboard text, or `null` when empty/denied. */
    fun readClipboard(): String?

    /** Write [text] and return a NativeClipboardWriteReceipt wire code. */
    fun writeClipboard(text: String): Int
}

internal sealed interface NativeClipboardWriteReceipt {
    val wireCode: Int

    data object Written : NativeClipboardWriteReceipt {
        override val wireCode = 0
    }

    data object Unavailable : NativeClipboardWriteReceipt {
        override val wireCode = 1
    }

    data class PermissionDenied(val detail: String) : NativeClipboardWriteReceipt {
        override val wireCode = 2
    }

    data class Failed(val detail: String) : NativeClipboardWriteReceipt {
        override val wireCode = 3
    }

    companion object {
        fun fromWireCode(code: Int): NativeClipboardWriteReceipt? = when (code) {
            0 -> Written
            1 -> Unavailable
            2 -> PermissionDenied("remote clipboard permission denied")
            3 -> Failed("remote clipboard write failed")
            else -> null
        }
    }
}

internal sealed interface NativeClipboardWriter {
    data object Unavailable : NativeClipboardWriter
    data class Available(val write: () -> Unit) : NativeClipboardWriter
}

internal fun attemptNativeClipboardWrite(write: () -> Unit): NativeClipboardWriteReceipt = try {
    write()
    NativeClipboardWriteReceipt.Written
} catch (error: SecurityException) {
    NativeClipboardWriteReceipt.PermissionDenied(error.message ?: error::class.java.simpleName)
} catch (error: Exception) {
    NativeClipboardWriteReceipt.Failed(error.message ?: error::class.java.simpleName)
}

internal fun performNativeClipboardWrite(writer: NativeClipboardWriter): NativeClipboardWriteReceipt = when (writer) {
    NativeClipboardWriter.Unavailable -> NativeClipboardWriteReceipt.Unavailable
    is NativeClipboardWriter.Available -> attemptNativeClipboardWrite(writer.write)
}

/** Activity instance callback into the Bevy NativeActivity's own Rust driver. */
fun interface BevyWindowInsetsPublisher {
    fun publish(json: String)
}

/** Exactly one process-local destination owns each Activity's inset updates. */
sealed interface NativeInsetsDelivery {
    data object ComposeRustHost : NativeInsetsDelivery
    data class BevyActivityFeed(val publisher: BevyWindowInsetsPublisher) : NativeInsetsDelivery
}

/**
 * BANDROID-005/006/008: the native Activity host adapter.
 *
 * It owns the real Android lifecycle, `WindowInsetsCompat` and
 * `ClipboardManager` wiring and publishes plain typed observations to the
 * explicitly selected UI-process consumer:
 *
 * - lifecycle (`onCreate`..`onDestroy`) and window focus become the shared
 *   `RenderCadence`; the Activity generation fences a retired instance so its
 *   late callbacks cannot overwrite the recreated one;
 * - status/navigation/cutout/system-gesture/IME insets are captured in
 *   physical pixels with the real display density; Compose delivers them to
 *   its Rust host registry, while Bevy exposes the observation directly to its
 *   own NativeActivity driver. The permanent safe area stays separate from
 *   the keyboard;
 * - the clipboard is exposed through [NativeHostBridge].
 *
 * BANDROID-007 is an explicit typed unsupported: this host does NOT install an
 * `InputConnection`/`GameTextInput`, because the locked toolchain
 * (`winit` 0.30.13 + `android-activity` 0.6.1 with the `native-activity`
 * feature + `bevy_android` 0.20) has no text-input channel into the Bevy
 * surface. The typed declaration lives in
 * `infiltrator_android::native_host::ime::android_ime_support()`; no fake IME
 * events are produced here.
 */
class NativeHostAdapter(private val activity: Activity, private val insetsDelivery: NativeInsetsDelivery) : NativeHostBridge {
    private val generation = GENERATION.getAndIncrement()

    @Volatile
    private var latestWindowInsetsJson: String? = null

    /**
     * 最近一次收到的窗口内边距观测。
     *
     * Surface 重建与旋转之后必须把这份观测重新发布一次：新 Surface 拿到
     * 的必须是当前的窗口事实，而不是上一次 Activity 留下的旧值。
     */
    private var latestInsets: WindowInsetsCompat? = null

    /** One typed lifecycle/focus observation; visibility is derived from phase. */
    private var lifecycleObservation = NativeHostLifecycleObservation(
        generation = generation,
        phase = NativeActivityPhase.Created,
        focus = NativeWindowFocus.Unknown,
    )

    private val clipboard: ClipboardManager?
        get() = activity.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager

    private val lifecycle = object : Application.ActivityLifecycleCallbacks {
        override fun onActivityCreated(owner: Activity, state: Bundle?) {
            if (owner === activity) updatePhase(NativeActivityPhase.Created)
        }

        override fun onActivityStarted(owner: Activity) {
            if (owner === activity) updatePhase(NativeActivityPhase.Started)
        }

        override fun onActivityResumed(owner: Activity) {
            if (owner === activity) updatePhase(NativeActivityPhase.Resumed)
        }

        override fun onActivityPaused(owner: Activity) {
            if (owner === activity) updatePhase(NativeActivityPhase.Paused)
        }

        override fun onActivityStopped(owner: Activity) {
            if (owner === activity) updatePhase(NativeActivityPhase.Stopped)
        }

        override fun onActivitySaveInstanceState(owner: Activity, state: Bundle) = Unit

        override fun onActivityDestroyed(owner: Activity) {
            if (owner === activity) updatePhase(NativeActivityPhase.Destroyed)
        }
    }

    private val decorView: View by lazy(LazyThreadSafetyMode.NONE) {
        activity.window.decorView
    }

    private val attachmentEffects = object : NativeHostAttachmentEffects {
        override fun registerLifecycleCallbacks(): NativeHostOperationResult {
            activity.application.registerActivityLifecycleCallbacks(lifecycle)
            return NativeHostOperationResult.Applied
        }

        override fun installInsetsListener(): NativeHostOperationResult {
            latestWindowInsetsJson = null
            latestInsets = null
            return installOwnedInsetsListener(decorView, generation) {
                ViewCompat.setOnApplyWindowInsetsListener(decorView) { _, insets ->
                    if (ownsInsetsListener(decorView, generation)) publishInsets(insets)
                    insets
                }
            }
        }

        override fun registerRustBridge(): NativeHostOperationResult = NATIVE_HOST_BRIDGE_OWNERS.register(generation) {
            nativeStatus(
                "nativeRegisterNativeHost",
                RustBridge.registerNativeHost(this@NativeHostAdapter),
            )
        }

        override fun publishInitialLifecycle(): NativeHostOperationResult = nativeStatus(
            "nativeOnLifecycle",
            RustBridge.onLifecycle(lifecycleObservation),
        )

        override fun requestInitialInsets(): NativeHostOperationResult {
            ViewCompat.requestApplyInsets(decorView)
            return NativeHostOperationResult.Applied
        }

        override fun unregisterLifecycleCallbacks(): NativeHostOperationResult {
            activity.application.unregisterActivityLifecycleCallbacks(lifecycle)
            return NativeHostOperationResult.Applied
        }

        override fun removeInsetsListener(): NativeHostOperationResult {
            val result = removeOwnedInsetsListener(decorView, generation)
            if (result == NativeHostOperationResult.Applied ||
                result == NativeHostOperationResult.Superseded
            ) {
                latestWindowInsetsJson = null
                latestInsets = null
            }
            return result
        }

        override fun clearRustBridge(): NativeHostOperationResult = NATIVE_HOST_BRIDGE_OWNERS.clearIfOwned(generation) {
            nativeStatus("nativeClearNativeHost", RustBridge.clearNativeHost())
        }
    }

    private val attachment = NativeHostAttachment(generation, attachmentEffects)

    /** Register lifecycle, Insets and Rust resources as one typed transaction. */
    internal fun attach(): NativeHostAttachOutcome {
        val outcome = attachment.attach()
        when (outcome) {
            is NativeHostAttachOutcome.Failed ->
                Log.e(TAG, "attach failed: ${outcome.failure}; cleanup=${outcome.cleanupFailures}")

            is NativeHostAttachOutcome.Rejected ->
                Log.e(TAG, "attach rejected in state ${outcome.state}")

            is NativeHostAttachOutcome.Attached,
            is NativeHostAttachOutcome.AlreadyAttached,
            -> Unit
        }
        return outcome
    }

    /** Release each resource independently; failed cleanup stays typed and retryable. */
    internal fun detach(): NativeHostDetachOutcome {
        if (attachment.acceptsCallbacks() &&
            lifecycleObservation.phase != NativeActivityPhase.Destroyed
        ) {
            updatePhase(NativeActivityPhase.Destroyed)
        }
        val outcome = attachment.detach()
        if (outcome is NativeHostDetachOutcome.CleanupNeeded) {
            Log.e(
                TAG,
                "detach needs cleanup for generation=${outcome.generation}, " +
                    "resources=${outcome.pendingResources}, failures=${outcome.failures}",
            )
        }
        return outcome
    }

    /**
     * Forward [Activity.onWindowFocusChanged]. Window focus is a fact distinct
     * from the lifecycle (dialogs/split screen), so it is published on its own.
     */
    fun onWindowFocusChanged(hasFocus: Boolean) {
        updateFocus(
            when (hasFocus) {
                true -> NativeWindowFocus.Focused
                false -> NativeWindowFocus.Unfocused
            },
        )
    }

    /**
     * EFFECTS-047 (#190): the platform created the Activity's render surface.
     *
     * The surface has no size yet, and the window facts of the previous
     * surface are stale: they are published again as soon as the new surface
     * is sized, so a recreated surface never keeps an outdated safe area.
     */
    internal fun onSurfaceCreated() {
        publishSurfaceState(NativeSurfaceState.Created, 0, 0)
    }

    /**
     * EFFECTS-047 (#190): the platform sized the Activity's render surface.
     */
    internal fun onSurfaceChanged(widthPx: Int, heightPx: Int) {
        publishSurfaceState(NativeSurfaceState.Live, widthPx, heightPx)
        republishWindowFacts()
    }

    /**
     * EFFECTS-047 (#190): the platform destroyed the Activity's render surface.
     *
     * Nothing may be rendered into a destroyed surface; the Rust owner keeps
     * the typed teardown so the shell stops presenting frames.
     */
    internal fun onSurfaceDestroyed() {
        publishSurfaceState(NativeSurfaceState.Destroyed, 0, 0)
    }

    /**
     * EFFECTS-047 (#192): the Activity published a new window configuration.
     *
     * The typed configuration fact goes to the Rust owner, and the window
     * metrics are published again so a rotated native surface lays out with
     * the current window instead of the size it had before the rotation.
     */
    internal fun onConfigurationChanged() {
        if (!attachment.acceptsCallbacks()) return
        val metrics = activity.resources.displayMetrics
        val fontScale = activity.resources.configuration.fontScale
        val widthPx = metrics.widthPixels
        val heightPx = metrics.heightPixels
        val orientation = nativeOrientationFor(widthPx, heightPx)
        val result = nativeStatus(
            "nativeOnConfiguration",
            RustBridge.onConfiguration(
                generation,
                orientation.wireCode,
                widthPx,
                heightPx,
                metrics.density,
                fontScale,
            ),
        )
        if (result is NativeHostOperationResult.Failed) {
            Log.e(TAG, "configuration publish failed: ${result.detail}")
        }
        republishWindowFacts()
    }

    private fun publishSurfaceState(state: NativeSurfaceState, widthPx: Int, heightPx: Int) {
        if (!attachment.acceptsCallbacks()) return
        val code = RustBridge.onSurface(generation, state.wireCode, widthPx, heightPx)
        when (nativeSurfaceReceiptFrom(code)) {
            NativeSurfaceReceipt.Accepted -> Unit

            NativeSurfaceReceipt.Stale ->
                Log.d(TAG, "surface fact from a retired Activity generation was ignored")

            NativeSurfaceReceipt.Failed ->
                Log.e(TAG, "surface publish failed with code $code")
        }
    }

    /**
     * Publish the current window facts again.
     *
     * A recreated surface and a rotated window both need the current safe
     * area and the current window metrics; without this the surface would
     * keep the facts of the previous one.
     */
    private fun republishWindowFacts() {
        val insets = latestInsets ?: return
        publishInsets(insets)
    }

    private fun updatePhase(next: NativeActivityPhase) {
        when (lifecycleObservation.phase.transitionTo(next)) {
            NativeActivityPhaseTransition.Unchanged -> return

            NativeActivityPhaseTransition.Rejected -> {
                Log.w(TAG, "rejected lifecycle transition: ${lifecycleObservation.phase} -> $next")
                return
            }

            NativeActivityPhaseTransition.Advanced -> Unit
        }
        lifecycleObservation = lifecycleObservation.copy(phase = next)
        publish()
    }

    private fun updateFocus(next: NativeWindowFocus) {
        if (!attachment.acceptsCallbacks() ||
            lifecycleObservation.phase == NativeActivityPhase.Destroyed ||
            lifecycleObservation.focus == next
        ) {
            return
        }
        lifecycleObservation = lifecycleObservation.copy(focus = next)
        publish()
    }

    private fun publish() {
        if (!attachment.acceptsCallbacks()) return
        val result = nativeStatus(
            "nativeOnLifecycle",
            RustBridge.onLifecycle(lifecycleObservation),
        )
        if (result is NativeHostOperationResult.Failed) {
            Log.e(TAG, "lifecycle publish failed: ${result.detail}")
        }
    }

    private fun publishInsets(insets: WindowInsetsCompat) {
        if (!attachment.acceptsCallbacks()) return
        latestInsets = insets
        val density = activity.resources.displayMetrics.density
        val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
        val status = insets.getInsets(WindowInsetsCompat.Type.statusBars())
        val navigation = insets.getInsets(WindowInsetsCompat.Type.navigationBars())
        val cutout = insets.getInsets(WindowInsetsCompat.Type.displayCutout())
        val systemGestures = insets.getInsets(WindowInsetsCompat.Type.systemGestures())
        val mandatoryGestures =
            insets.getInsets(WindowInsetsCompat.Type.mandatorySystemGestures())
        val gestureSafeArea = Insets.of(
            maxOf(systemGestures.left, mandatoryGestures.left),
            maxOf(systemGestures.top, mandatoryGestures.top),
            maxOf(systemGestures.right, mandatoryGestures.right),
            maxOf(systemGestures.bottom, mandatoryGestures.bottom),
        )
        val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
        when (insetsDelivery) {
            NativeInsetsDelivery.ComposeRustHost -> {
                latestWindowInsetsJson = null
                val result = nativeStatus(
                    "nativeOnInsets",
                    RustBridge.onInsets(
                        density,
                        maxOf(bars.top, cutout.top),
                        maxOf(bars.right, cutout.right),
                        maxOf(bars.bottom, cutout.bottom),
                        maxOf(bars.left, cutout.left),
                        ime.top,
                        ime.right,
                        ime.bottom,
                        ime.left,
                    ),
                )
                if (result is NativeHostOperationResult.Failed) {
                    Log.e(TAG, "Compose Insets publish failed: ${result.detail}")
                }
            }

            is NativeInsetsDelivery.BevyActivityFeed -> {
                val displayMetrics = activity.resources.displayMetrics
                val widthDp = (displayMetrics.widthPixels / density).toDouble()
                val heightDp = (displayMetrics.heightPixels / density).toDouble()
                val fontScale = activity.resources.configuration.fontScale.toDouble()
                val observation = JSONObject()
                    .put("activity_generation", generation)
                    .put("density", density.toDouble())
                    .put("width_dp", widthDp)
                    .put("height_dp", heightDp)
                    .put("font_scale", fontScale)
                    .put("status_bar", edgesJson(status))
                    .put("nav_bar", edgesJson(navigation))
                    .put("cutout", edgesJson(cutout))
                    .put("system_gestures", edgesJson(gestureSafeArea))
                    .put("ime", edgesJson(ime))
                    .toString()
                if (latestWindowInsetsJson == observation) return
                try {
                    insetsDelivery.publisher.publish(observation)
                    latestWindowInsetsJson = observation
                } catch (error: Exception) {
                    Log.e(TAG, "Bevy Insets publish failed: ${error.message}", error)
                }
            }
        }
    }

    /** Latest complete typed inset observation for the Bevy NativeActivity. */
    fun currentWindowInsetsJson(): String? = if (attachment.acceptsCallbacks()) latestWindowInsetsJson else null

    private fun edgesJson(insets: Insets): JSONObject = JSONObject()
        .put("top", insets.top)
        .put("right", insets.right)
        .put("bottom", insets.bottom)
        .put("left", insets.left)

    override fun readClipboard(): String? = try {
        val clip = clipboard?.primaryClip
        if (clip == null || clip.itemCount == 0) {
            null
        } else {
            clip.getItemAt(0).coerceToText(activity)?.toString()
        }
    } catch (error: Exception) {
        Log.w(TAG, "clipboard read failed: ${error.message}")
        null
    }

    override fun writeClipboard(text: String): Int = writeClipboardReceipt(text).wireCode

    private fun writeClipboardReceipt(text: String): NativeClipboardWriteReceipt {
        val receipt = try {
            val writer = when (val manager = clipboard) {
                null -> NativeClipboardWriter.Unavailable

                else -> NativeClipboardWriter.Available {
                    manager.setPrimaryClip(ClipData.newPlainText(CLIP_LABEL, text))
                }
            }
            performNativeClipboardWrite(writer)
        } catch (error: Exception) {
            when (error) {
                is SecurityException ->
                    NativeClipboardWriteReceipt.PermissionDenied(error.message ?: error::class.java.simpleName)

                else -> NativeClipboardWriteReceipt.Failed(error.message ?: error::class.java.simpleName)
            }
        }
        when (receipt) {
            NativeClipboardWriteReceipt.Written,
            NativeClipboardWriteReceipt.Unavailable,
            -> Unit

            is NativeClipboardWriteReceipt.PermissionDenied ->
                Log.w(TAG, "clipboard write permission denied: ${receipt.detail}")

            is NativeClipboardWriteReceipt.Failed ->
                Log.w(TAG, "clipboard write failed: ${receipt.detail}")
        }
        return receipt
    }

    companion object {
        private const val TAG = "NativeHostAdapter"
        private const val CLIP_LABEL = "MusicFrog"

        private val GENERATION = AtomicLong(0)
        private val NATIVE_HOST_BRIDGE_OWNERS = NativeHostGenerationRegistry()
        private val INSETS_OWNER_LOCK = Any()
        private val INSETS_LISTENER_OWNERS = WeakHashMap<View, Long>()

        private fun nativeStatus(operation: String, code: Int): NativeHostOperationResult = if (code == 0) {
            NativeHostOperationResult.Applied
        } else {
            NativeHostOperationResult.Failed("$operation returned status code $code")
        }

        private fun installOwnedInsetsListener(view: View, generation: Long, install: () -> Unit): NativeHostOperationResult =
            synchronized(INSETS_OWNER_LOCK) {
                val owner = INSETS_LISTENER_OWNERS[view]
                if (owner != null && owner != generation) {
                    return@synchronized NativeHostOperationResult.Failed(
                        "WindowInsets listener is still owned by Activity generation $owner",
                    )
                }
                // Claim before the platform call so rollback can remove a listener
                // even if the API throws after partially installing it.
                INSETS_LISTENER_OWNERS[view] = generation
                install()
                NativeHostOperationResult.Applied
            }

        private fun ownsInsetsListener(view: View, generation: Long): Boolean = synchronized(INSETS_OWNER_LOCK) {
            INSETS_LISTENER_OWNERS[view] == generation
        }

        private fun removeOwnedInsetsListener(view: View, generation: Long): NativeHostOperationResult = synchronized(INSETS_OWNER_LOCK) {
            if (INSETS_LISTENER_OWNERS[view] != generation) {
                return@synchronized NativeHostOperationResult.Superseded
            }
            ViewCompat.setOnApplyWindowInsetsListener(view, null)
            INSETS_LISTENER_OWNERS.remove(view)
            NativeHostOperationResult.Applied
        }
    }
}
