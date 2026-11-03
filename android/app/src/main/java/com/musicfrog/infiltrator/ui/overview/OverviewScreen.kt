package com.musicfrog.infiltrator.ui.overview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.infiltrator.AndroidActivityState
import com.musicfrog.infiltrator.AndroidVpnAuthorizationUiState
import com.musicfrog.infiltrator.VpnOwnerDataPlaneState
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.OwnerVpnObservation
import infiltrator_android.AndroidControlIntent

@Composable
internal fun OverviewScreen(
    vpnAuthorization: AndroidVpnAuthorizationUiState,
    onRequestVpnPermission: () -> Unit,
    onControlIntent: (AndroidControlIntent) -> Unit,
    isExpanded: Boolean,
    onNavigateToLogs: () -> Unit = {},
    viewModel: OverviewViewModel = viewModel(),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val activityError by AndroidActivityState.error.collectAsStateWithLifecycle()

    val displayError = state.error ?: state.coreLifecycleError ?: state.vpnStateError ?: activityError

    if (displayError != null) {
        ErrorDialog(
            failure = displayError,
            onDismiss = { viewModel.clearError() },
        )
    }

    val cards = remember(state, vpnAuthorization) {
        overviewCards(
            coreLifecycle = state.coreLifecycle,
            vpnOwnerObservation = state.vpnOwnerObservation,
            vpnAuthorization = vpnAuthorization,
            routingIntent = state.routingIntent,
            traffic = state.traffic,
            logs = state.logs,
            egressIp = state.egressIp,
            onRequestVpnPermission = onRequestVpnPermission,
            onToggleVpn = {
                when (val observation = state.vpnOwnerObservation) {
                    is ObservationState.Loading,
                    ObservationState.Empty,
                    is ObservationState.Unavailable,
                    -> Unit

                    is ObservationState.Observed -> when (val vpn = observation.value) {
                        is OwnerVpnObservation.AndroidOwner -> when (vpn.snapshot.dataPlaneState()) {
                            VpnOwnerDataPlaneState.Running ->
                                onControlIntent(AndroidControlIntent.STOP_PROXY)

                            VpnOwnerDataPlaneState.PermissionRequired,
                            VpnOwnerDataPlaneState.Stopped,
                            VpnOwnerDataPlaneState.Revoked,
                            -> onRequestVpnPermission()

                            is VpnOwnerDataPlaneState.Failed -> if (
                                canStartVpnSession(observation, vpnAuthorization)
                            ) {
                                onRequestVpnPermission()
                            }

                            VpnOwnerDataPlaneState.Starting,
                            VpnOwnerDataPlaneState.Stopping,
                            is VpnOwnerDataPlaneState.CleanupPending,
                            is VpnOwnerDataPlaneState.Unavailable,
                            -> Unit
                        }

                        is OwnerVpnObservation.Unsupported,
                        OwnerVpnObservation.NotApplicable,
                        is OwnerVpnObservation.Unavailable,
                        -> Unit
                    }
                }
            },
            onResumeCore = { onControlIntent(AndroidControlIntent.RESUME_KERNEL) },
            onRestartCore = { onControlIntent(AndroidControlIntent.RESTART_KERNEL) },
            onRoutingIntentChange = viewModel::changeRoutingIntent,
            onRefreshIp = { viewModel.refreshIpCheck() },
            onNavigateToLogs = onNavigateToLogs,
        )
    }

    val contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)

    if (isExpanded) {
        val leftCards = cards.filterIndexed { index, _ -> index % 2 == 0 }
        val rightCards = cards.filterIndexed { index, _ -> index % 2 == 1 }
        Row(
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            modifier = Modifier
                .fillMaxSize()
                .padding(contentPadding),
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                leftCards.forEach { card -> card() }
            }
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                rightCards.forEach { card -> card() }
            }
        }
    } else {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = contentPadding,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(cards) { card ->
                card()
            }
            item { Spacer(modifier = Modifier.height(16.dp)) }
        }
    }
}
