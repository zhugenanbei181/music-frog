package com.musicfrog.infiltrator

import android.content.Context
import com.musicfrog.infiltrator.ui.common.LocalizedFailure
import infiltrator_android.VpnAuthorizationResult
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

enum class AndroidVpnAuthorizationUiState {
    Unknown,
    Required,
    Granted,
    Unavailable,
}

/** Activity-only projection for Android-owned consent and transient user feedback. */
internal object AndroidActivityState {
    private val _vpnAuthorization = MutableStateFlow(AndroidVpnAuthorizationUiState.Unknown)
    val vpnAuthorization: StateFlow<AndroidVpnAuthorizationUiState> = _vpnAuthorization.asStateFlow()

    private val _error = MutableStateFlow<LocalizedFailure?>(null)
    val error: StateFlow<LocalizedFailure?> = _error.asStateFlow()

    fun refreshVpnAuthorization(context: Context): AndroidVpnAuthorizationUiState {
        return applyVpnAuthorizationAvailability(readVpnAuthorizationAvailability(context))
    }

    fun applyVpnAuthorizationAvailability(
        availability: VpnAuthorizationAvailability,
    ): AndroidVpnAuthorizationUiState {
        val state = when (availability) {
            VpnAuthorizationAvailability.Granted -> AndroidVpnAuthorizationUiState.Granted
            VpnAuthorizationAvailability.Required -> AndroidVpnAuthorizationUiState.Required
            is VpnAuthorizationAvailability.Unavailable -> AndroidVpnAuthorizationUiState.Unavailable
        }
        _vpnAuthorization.value = state
        return state
    }

    fun applyVpnAuthorizationResult(result: VpnAuthorizationResult) {
        _vpnAuthorization.value = when (result) {
            VpnAuthorizationResult.GRANTED -> AndroidVpnAuthorizationUiState.Granted
            VpnAuthorizationResult.DENIED,
            VpnAuthorizationResult.REVOKED -> AndroidVpnAuthorizationUiState.Required
        }
    }

    fun reportError(failure: LocalizedFailure) {
        _error.value = failure
    }

    fun clearError() {
        _error.value = null
    }
}
