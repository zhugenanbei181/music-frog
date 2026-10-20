package com.musicfrog.infiltrator

import android.content.Context
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import infiltrator_android.VpnAuthorizationResult

internal sealed interface VpnAuthorizationAvailability {
    data object Granted : VpnAuthorizationAvailability

    data object Required : VpnAuthorizationAvailability

    data class Unavailable(val message: String) : VpnAuthorizationAvailability
}

internal fun readVpnAuthorizationAvailability(context: Context): VpnAuthorizationAvailability = try {
    when (VpnService.prepare(context)) {
        null -> VpnAuthorizationAvailability.Granted
        else -> VpnAuthorizationAvailability.Required
    }
} catch (error: Exception) {
    VpnAuthorizationAvailability.Unavailable(
        error.message ?: "VPN authorization state could not be read",
    )
}

internal sealed interface VpnConsentPromptLaunch {
    data object Launched : VpnConsentPromptLaunch

    data object AuthorizationAlreadyGranted : VpnConsentPromptLaunch

    data class Failed(val message: String) : VpnConsentPromptLaunch
}

internal sealed interface NotificationPromptLaunch {
    data object Launched : NotificationPromptLaunch

    data object PermissionAlreadyGranted : NotificationPromptLaunch

    data class Failed(val message: String) : NotificationPromptLaunch
}

internal enum class AndroidVpnAuthorizationFlowState {
    Idle,
    AwaitingVpnConsent,
    AwaitingNotificationPermission,
    DispatchingStart,
}

internal const val VPN_AUTHORIZATION_FLOW_STATE_KEY =
    "com.musicfrog.infiltrator.VPN_AUTHORIZATION_FLOW_STATE"

internal sealed interface VpnProxyStartReadback {
    data object Running : VpnProxyStartReadback

    data class CommandRejected(val code: FfiErrorCode, val message: String) : VpnProxyStartReadback

    data class OwnerStateUnavailable(val message: String) : VpnProxyStartReadback

    data class OwnerFailed(val message: String) : VpnProxyStartReadback

    data class OwnerNotRunning(val snapshot: VpnOwnerSnapshotWire, val state: VpnOwnerDataPlaneState) : VpnProxyStartReadback
}

internal interface AndroidVpnAuthorizationHost {
    fun authorizationAvailability(): VpnAuthorizationAvailability

    fun launchVpnConsentPrompt(): VpnConsentPromptLaunch

    fun notificationPermissionStatus(): NotificationPermissionStatus

    fun launchNotificationPermissionPrompt(): NotificationPromptLaunch

    fun reportVpnAuthorization(result: VpnAuthorizationResult): FfiStatus

    fun onVpnAuthorizationNotGranted(status: FfiStatus)

    fun onNotificationPermissionNotGranted(result: NotificationPermissionPromptResult)

    fun onNotificationPermissionPromptFailed(message: String)

    fun dispatchProxyStart(completion: (VpnProxyStartReadback) -> Unit)

    fun reportFailure(message: String)
}

/**
 * Shared Compose/Bevy authorization flow. Platform Activities only adapt their
 * result-launch APIs; prompt ordering, return handling, and start gating
 * are owned here.
 */
internal class AndroidVpnAuthorizationFlow(
    private val host: AndroidVpnAuthorizationHost,
    restoredState: AndroidVpnAuthorizationFlowState = AndroidVpnAuthorizationFlowState.Idle,
) {
    private var current = stateAfterRecreation(restoredState)

    fun state(): AndroidVpnAuthorizationFlowState = current

    fun stateForSaving(): AndroidVpnAuthorizationFlowState = when (current) {
        AndroidVpnAuthorizationFlowState.AwaitingVpnConsent,
        AndroidVpnAuthorizationFlowState.AwaitingNotificationPermission,
        -> current

        AndroidVpnAuthorizationFlowState.Idle,
        AndroidVpnAuthorizationFlowState.DispatchingStart,
        -> AndroidVpnAuthorizationFlowState.Idle
    }

    fun requestStart() {
        if (current != AndroidVpnAuthorizationFlowState.Idle) return
        when (val availability = host.authorizationAvailability()) {
            VpnAuthorizationAvailability.Granted -> reportGrantedAuthorization()
            VpnAuthorizationAvailability.Required -> requestVpnConsent()
            is VpnAuthorizationAvailability.Unavailable -> fail(availability.message)
        }
    }

    fun onVpnConsentPromptReturned() {
        if (current != AndroidVpnAuthorizationFlowState.AwaitingVpnConsent) return
        when (val availability = host.authorizationAvailability()) {
            VpnAuthorizationAvailability.Granted -> reportGrantedAuthorization()
            VpnAuthorizationAvailability.Required -> reportAuthorizationNotGranted()
            is VpnAuthorizationAvailability.Unavailable -> fail(availability.message)
        }
    }

    fun onNotificationPermissionPromptReturned(result: NotificationPermissionPromptResult) {
        if (current != AndroidVpnAuthorizationFlowState.AwaitingNotificationPermission) return
        if (result == NotificationPermissionPromptResult.NotGranted) {
            host.onNotificationPermissionNotGranted(result)
        }
        dispatchProxyStart()
    }

    private fun requestVpnConsent() {
        current = AndroidVpnAuthorizationFlowState.AwaitingVpnConsent
        when (val launch = host.launchVpnConsentPrompt()) {
            VpnConsentPromptLaunch.Launched -> Unit
            VpnConsentPromptLaunch.AuthorizationAlreadyGranted -> onVpnConsentPromptReturned()
            is VpnConsentPromptLaunch.Failed -> fail(launch.message)
        }
    }

    private fun reportGrantedAuthorization() {
        val status = try {
            host.reportVpnAuthorization(VpnAuthorizationResult.GRANTED)
        } catch (error: Exception) {
            fail(error.message ?: "VPN authorization could not be recorded")
            return
        }
        if (status.code != FfiErrorCode.OK) {
            fail(status.message ?: "VPN authorization could not be recorded")
            return
        }
        continueAfterAuthorization()
    }

    private fun reportAuthorizationNotGranted() {
        val status = try {
            host.reportVpnAuthorization(VpnAuthorizationResult.DENIED)
        } catch (error: Exception) {
            fail(error.message ?: "VPN authorization result could not be recorded")
            return
        }
        current = AndroidVpnAuthorizationFlowState.Idle
        host.onVpnAuthorizationNotGranted(status)
    }

    private fun continueAfterAuthorization() {
        when (host.notificationPermissionStatus()) {
            NotificationPermissionStatus.NotRequired,
            NotificationPermissionStatus.Granted,
            -> dispatchProxyStart()

            NotificationPermissionStatus.NotGranted -> requestNotificationPermission()
        }
    }

    private fun requestNotificationPermission() {
        current = AndroidVpnAuthorizationFlowState.AwaitingNotificationPermission
        when (val launch = host.launchNotificationPermissionPrompt()) {
            NotificationPromptLaunch.Launched -> Unit

            NotificationPromptLaunch.PermissionAlreadyGranted -> dispatchProxyStart()

            is NotificationPromptLaunch.Failed -> {
                host.onNotificationPermissionPromptFailed(launch.message)
                dispatchProxyStart()
            }
        }
    }

    private fun dispatchProxyStart() {
        current = AndroidVpnAuthorizationFlowState.DispatchingStart
        try {
            host.dispatchProxyStart(::onProxyStartReadback)
        } catch (error: Exception) {
            fail(error.message ?: "VPN service start could not be requested")
        }
    }

    private fun onProxyStartReadback(result: VpnProxyStartReadback) {
        if (current != AndroidVpnAuthorizationFlowState.DispatchingStart) return
        current = AndroidVpnAuthorizationFlowState.Idle
        when (result) {
            VpnProxyStartReadback.Running -> Unit

            is VpnProxyStartReadback.CommandRejected -> host.reportFailure(
                "VPN start command failed (${result.code}): ${result.message}",
            )

            is VpnProxyStartReadback.OwnerStateUnavailable -> host.reportFailure(result.message)

            is VpnProxyStartReadback.OwnerFailed -> host.reportFailure(result.message)

            is VpnProxyStartReadback.OwnerNotRunning -> host.reportFailure(
                "The :vpn owner did not confirm a foreground running VPN session " +
                    "(state=${result.state}, generation=${result.snapshot.vpnGeneration}, " +
                    "revision=${result.snapshot.revision})",
            )
        }
    }

    private fun fail(message: String) {
        current = AndroidVpnAuthorizationFlowState.Idle
        host.reportFailure(message)
    }

    companion object {
        fun stateAfterRecreation(state: AndroidVpnAuthorizationFlowState): AndroidVpnAuthorizationFlowState = when (state) {
            AndroidVpnAuthorizationFlowState.AwaitingVpnConsent,
            AndroidVpnAuthorizationFlowState.AwaitingNotificationPermission,
            -> state

            AndroidVpnAuthorizationFlowState.Idle,
            AndroidVpnAuthorizationFlowState.DispatchingStart,
            -> AndroidVpnAuthorizationFlowState.Idle
        }
    }
}

internal fun classifyVpnOwnerStartReadback(snapshot: VpnOwnerSnapshotWire): VpnProxyStartReadback =
    when (val state = snapshot.dataPlaneState()) {
        VpnOwnerDataPlaneState.Running -> VpnProxyStartReadback.Running

        is VpnOwnerDataPlaneState.Failed -> VpnProxyStartReadback.OwnerFailed(state.failureMessage)

        is VpnOwnerDataPlaneState.Unavailable ->
            VpnProxyStartReadback.OwnerStateUnavailable(state.failureMessage)

        VpnOwnerDataPlaneState.PermissionRequired,
        VpnOwnerDataPlaneState.Starting,
        VpnOwnerDataPlaneState.Stopping,
        is VpnOwnerDataPlaneState.CleanupPending,
        VpnOwnerDataPlaneState.Stopped,
        VpnOwnerDataPlaneState.Revoked,
        -> VpnProxyStartReadback.OwnerNotRunning(snapshot, state)
    }

internal fun savedVpnAuthorizationFlowState(savedState: Bundle?): AndroidVpnAuthorizationFlowState {
    val restored = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        savedState?.getSerializable(
            VPN_AUTHORIZATION_FLOW_STATE_KEY,
            AndroidVpnAuthorizationFlowState::class.java,
        )
    } else {
        @Suppress("DEPRECATION")
        savedState?.getSerializable(VPN_AUTHORIZATION_FLOW_STATE_KEY)
            as? AndroidVpnAuthorizationFlowState
    } ?: AndroidVpnAuthorizationFlowState.Idle
    return AndroidVpnAuthorizationFlow.stateAfterRecreation(restored)
}
