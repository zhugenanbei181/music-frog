package com.musicfrog.infiltrator

import android.app.Activity
import android.app.Application
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.util.Log
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import java.util.concurrent.atomic.AtomicLong

/**
 * BANDROID-008: the clipboard channel Rust calls back into through JNI. The
 * Activity owns the real `ClipboardManager`; no `Context`/`ClipData` type
 * crosses the shared port boundary.
 */
interface NativeHostBridge {
    /** The current clipboard text, or `null` when empty/denied. */
    fun readClipboard(): String?

    /** Write [text]; returns whether the platform accepted it. */
    fun writeClipboard(text: String): Boolean
}

/**
 * BANDROID-005/006/008: the native Activity host adapter.
 *
 * It owns the real Android lifecycle, `WindowInsetsCompat` and
 * `ClipboardManager` wiring and pushes only plain values into Rust:
 *
 * - lifecycle (`onCreate`..`onDestroy`) and window focus become the shared
 *   `RenderCadence`; the Activity generation fences a retired instance so its
 *   late callbacks cannot overwrite the recreated one;
 * - status/navigation/cutout/IME insets are pushed in physical pixels with the
 *   real display density, keeping the permanent safe area separate from the
 *   keyboard;
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
class NativeHostAdapter(private val activity: Activity) : NativeHostBridge {
    private val generation = GENERATION.getAndIncrement()
    private var attached = false

    /** `onStart`..`onStop` visibility. */
    private var visible = false

    /** Window focus, tracked independently of the lifecycle. */
    private var focused = true

    /** The last Activity phase, sent with focus-only updates. */
    private var phase = PHASE_CREATED

    private val clipboard: ClipboardManager? =
        activity.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager

    private val lifecycle = object : Application.ActivityLifecycleCallbacks {
        override fun onActivityCreated(owner: Activity, state: Bundle?) {
            if (owner === activity) updatePhase(PHASE_CREATED)
        }

        override fun onActivityStarted(owner: Activity) {
            if (owner === activity) {
                visible = true
                updatePhase(PHASE_STARTED)
            }
        }

        override fun onActivityResumed(owner: Activity) {
            if (owner === activity) {
                visible = true
                updatePhase(PHASE_RESUMED)
            }
        }

        override fun onActivityPaused(owner: Activity) {
            if (owner === activity) updatePhase(PHASE_PAUSED)
        }

        override fun onActivityStopped(owner: Activity) {
            if (owner === activity) {
                visible = false
                updatePhase(PHASE_STOPPED)
            }
        }

        override fun onActivitySaveInstanceState(owner: Activity, state: Bundle) = Unit

        override fun onActivityDestroyed(owner: Activity) {
            if (owner === activity) {
                visible = false
                updatePhase(PHASE_DESTROYED)
            }
        }
    }

    /** Register the lifecycle/insets listeners and the clipboard channel once. */
    fun attach() {
        if (attached) return
        attached = true
        activity.application.registerActivityLifecycleCallbacks(lifecycle)
        ViewCompat.setOnApplyWindowInsetsListener(activity.window.decorView) { _, insets ->
            publishInsets(insets)
            insets
        }
        val registered = RustBridge.registerNativeHost(this)
        if (registered != 0) {
            Log.w(TAG, "native host registration failed: $registered")
        }
        // The Activity already passed onCreate before this adapter attached.
        updatePhase(PHASE_CREATED)
        ViewCompat.requestApplyInsets(activity.window.decorView)
    }

    /** Release listeners and the clipboard global ref. Idempotent. */
    fun detach() {
        if (!attached) return
        attached = false
        activity.application.unregisterActivityLifecycleCallbacks(lifecycle)
        ViewCompat.setOnApplyWindowInsetsListener(activity.window.decorView, null)
        RustBridge.clearNativeHost()
    }

    /**
     * Forward [Activity.onWindowFocusChanged]. Window focus is a fact distinct
     * from the lifecycle (dialogs/split screen), so it is published on its own.
     */
    fun onWindowFocusChanged(hasFocus: Boolean) {
        if (!attached) return
        focused = hasFocus
        publish()
    }

    private fun updatePhase(next: Int) {
        phase = next
        publish()
    }

    private fun publish() {
        RustBridge.onLifecycle(generation, phase, focused, visible)
    }

    private fun publishInsets(insets: WindowInsetsCompat) {
        val density = activity.resources.displayMetrics.density
        val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
        val cutout = insets.getInsets(WindowInsetsCompat.Type.displayCutout())
        val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
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
        )
    }

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

    override fun writeClipboard(text: String): Boolean = try {
        val manager = clipboard ?: return false
        manager.setPrimaryClip(ClipData.newPlainText(CLIP_LABEL, text))
        true
    } catch (error: Exception) {
        Log.w(TAG, "clipboard write failed: ${error.message}")
        false
    }

    companion object {
        private const val TAG = "NativeHostAdapter"
        private const val CLIP_LABEL = "MusicFrog"

        // Wire codes mirrored by `infiltrator_android::native_host::lifecycle`.
        private const val PHASE_CREATED = 0
        private const val PHASE_STARTED = 1
        private const val PHASE_RESUMED = 2
        private const val PHASE_PAUSED = 3
        private const val PHASE_STOPPED = 4
        private const val PHASE_DESTROYED = 5

        private val GENERATION = AtomicLong(0)
    }
}
