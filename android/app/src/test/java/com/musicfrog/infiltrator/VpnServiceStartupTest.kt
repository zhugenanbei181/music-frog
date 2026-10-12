package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class VpnServiceStartupTest {
    @Test
    fun explicitUserCommandsRemainIndependentOfSystemAlwaysOnPolicy() {
        assertEquals(
            VpnServiceStartupDecision.ExecuteCommand(
                VpnServiceCommand.Start,
                VpnServiceDelivery.Initial,
                VpnServiceNotificationActionMode.UserManaged,
            ),
            decideVpnServiceStartup(
                VpnServiceCommandExtra.Present(VpnServiceCommand.Start),
                VpnServiceDelivery.Initial,
                VpnAlwaysOnObservation.Disabled,
            ),
        )
        assertEquals(
            VpnServiceStartupDecision.ExecuteCommand(
                VpnServiceCommand.Stop,
                VpnServiceDelivery.Initial,
                VpnServiceNotificationActionMode.SystemManaged,
            ),
            decideVpnServiceStartup(
                VpnServiceCommandExtra.Present(VpnServiceCommand.Stop),
                VpnServiceDelivery.Initial,
                VpnAlwaysOnObservation.Enabled,
            ),
        )
    }

    @Test
    fun unflaggedSystemStartRestoresAlwaysOnAndKeepsRedeliveryTyped() {
        assertEquals(
            VpnServiceStartupDecision.RestoreSystemManagedProxy(
                VpnSystemManagedPolicy.AlwaysOnConfirmed,
                VpnServiceDelivery.Redelivered,
            ),
            decideVpnServiceStartup(
                VpnServiceCommandExtra.Missing,
                VpnServiceDelivery.Redelivered,
                VpnAlwaysOnObservation.Enabled,
            ),
        )
    }

    @Test
    fun olderAndroidUsesOnlyUnflaggedSystemStartForAlwaysOnRecovery() {
        assertEquals(
            VpnServiceStartupDecision.RestoreSystemManagedProxy(
                VpnSystemManagedPolicy.LegacyUnflaggedStart,
                VpnServiceDelivery.Initial,
            ),
            decideVpnServiceStartup(
                VpnServiceCommandExtra.Missing,
                VpnServiceDelivery.Initial,
                VpnAlwaysOnObservation.NotQueryable,
            ),
        )
        assertEquals(
            VpnServiceStartupDecision.Reject(VpnServiceStartupRejection.MalformedCommand),
            decideVpnServiceStartup(
                VpnServiceCommandExtra.Malformed,
                VpnServiceDelivery.Initial,
                VpnAlwaysOnObservation.NotQueryable,
            ),
        )
    }

    @Test
    fun anUnflaggedStartCannotWakeVpnWhenAlwaysOnIsDisabled() {
        assertEquals(
            VpnServiceStartupDecision.Reject(VpnServiceStartupRejection.AlwaysOnNotEnabled),
            decideVpnServiceStartup(
                VpnServiceCommandExtra.Missing,
                VpnServiceDelivery.Initial,
                VpnAlwaysOnObservation.Disabled,
            ),
        )
    }

    @Test
    fun ownerWaitReturnsTypedReadyFailureAndTimeoutOutcomes() {
        assertEquals(
            VpnOwnerSessionWaitDirective.Complete(VpnOwnerSessionWaitOutcome.Ready("owner-1")),
            decideVpnOwnerSessionWait(
                VpnOwnerSessionReadiness.Ready("owner-1"),
                VpnOwnerSessionWaitBudget.Remaining(500L),
                pollIntervalMillis = 50L,
            ),
        )
        assertEquals(
            VpnOwnerSessionWaitDirective.Complete(
                VpnOwnerSessionWaitOutcome.Failed("initialization failed"),
            ),
            decideVpnOwnerSessionWait(
                VpnOwnerSessionReadiness.Failed("owner-1", "initialization failed"),
                VpnOwnerSessionWaitBudget.Remaining(500L),
                pollIntervalMillis = 50L,
            ),
        )
        assertEquals(
            VpnOwnerSessionWaitDirective.Complete(VpnOwnerSessionWaitOutcome.TimedOut),
            decideVpnOwnerSessionWait(
                VpnOwnerSessionReadiness.Preparing("owner-1"),
                VpnOwnerSessionWaitBudget.Expired,
                pollIntervalMillis = 50L,
            ),
        )
    }

    @Test
    fun ownerWaitUsesTheSmallerOfDeadlineAndPollInterval() {
        assertEquals(
            VpnOwnerSessionWaitDirective.Wait(17L),
            decideVpnOwnerSessionWait(
                VpnOwnerSessionReadiness.NotStarted,
                VpnOwnerSessionWaitBudget.Remaining(17L),
                pollIntervalMillis = 50L,
            ),
        )
        assertEquals(
            VpnOwnerSessionWaitDirective.Wait(50L),
            decideVpnOwnerSessionWait(
                VpnOwnerSessionReadiness.Preparing("owner-1"),
                VpnOwnerSessionWaitBudget.Remaining(500L),
                pollIntervalMillis = 50L,
            ),
        )
    }
}
