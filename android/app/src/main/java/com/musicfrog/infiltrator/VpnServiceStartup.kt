package com.musicfrog.infiltrator

internal enum class VpnServiceCommand {
    Start,
    Stop,
}

internal sealed interface VpnServiceCommandExtra {
    data object Missing : VpnServiceCommandExtra
    data object Malformed : VpnServiceCommandExtra
    data class Present(val command: VpnServiceCommand) : VpnServiceCommandExtra
}

internal enum class VpnServiceDelivery {
    Initial,
    Redelivered,
    Retried,
}

internal enum class VpnAlwaysOnObservation {
    Enabled,
    Disabled,
    NotQueryable,
}

internal enum class VpnServiceNotificationActionMode {
    UserManaged,
    SystemManaged,
}

internal enum class VpnSystemManagedPolicy {
    AlwaysOnConfirmed,
    LegacyUnflaggedStart,
}

internal enum class VpnServiceStartupRejection {
    MalformedCommand,
    AlwaysOnNotEnabled,
}

internal sealed interface VpnForegroundPromotion {
    data object Promoted : VpnForegroundPromotion
    data class Failed(val reason: String) : VpnForegroundPromotion
}

internal sealed interface VpnOwnerSessionWaitOutcome {
    data class Ready(val sessionId: String) : VpnOwnerSessionWaitOutcome
    data class Failed(val reason: String) : VpnOwnerSessionWaitOutcome
    data object TimedOut : VpnOwnerSessionWaitOutcome
    data object Interrupted : VpnOwnerSessionWaitOutcome
}

internal sealed interface VpnOwnerSessionWaitDirective {
    data class Complete(val outcome: VpnOwnerSessionWaitOutcome) : VpnOwnerSessionWaitDirective
    data class Wait(val millis: Long) : VpnOwnerSessionWaitDirective {
        init {
            require(millis > 0L)
        }
    }
}

internal sealed interface VpnOwnerSessionWaitBudget {
    data object Expired : VpnOwnerSessionWaitBudget
    data class Remaining(val millis: Long) : VpnOwnerSessionWaitBudget {
        init {
            require(millis > 0L)
        }
    }
}

internal fun decideVpnOwnerSessionWait(
    readiness: VpnOwnerSessionReadiness,
    budget: VpnOwnerSessionWaitBudget,
    pollIntervalMillis: Long,
): VpnOwnerSessionWaitDirective {
    require(pollIntervalMillis > 0L)
    return when (readiness) {
        is VpnOwnerSessionReadiness.Ready -> VpnOwnerSessionWaitDirective.Complete(
            VpnOwnerSessionWaitOutcome.Ready(readiness.sessionId),
        )

        is VpnOwnerSessionReadiness.Failed -> VpnOwnerSessionWaitDirective.Complete(
            VpnOwnerSessionWaitOutcome.Failed(readiness.reason),
        )

        is VpnOwnerSessionReadiness.Ended -> VpnOwnerSessionWaitDirective.Complete(
            VpnOwnerSessionWaitOutcome.Failed(
                "Core owner session ${readiness.sessionId} ended during startup",
            ),
        )

        VpnOwnerSessionReadiness.NotStarted,
        is VpnOwnerSessionReadiness.Preparing,
        -> when (budget) {
            VpnOwnerSessionWaitBudget.Expired -> VpnOwnerSessionWaitDirective.Complete(
                VpnOwnerSessionWaitOutcome.TimedOut,
            )

            is VpnOwnerSessionWaitBudget.Remaining -> VpnOwnerSessionWaitDirective.Wait(
                minOf(budget.millis, pollIntervalMillis),
            )
        }
    }
}

internal sealed interface VpnServiceStartupDecision {
    data class ExecuteCommand(
        val command: VpnServiceCommand,
        val delivery: VpnServiceDelivery,
        val notificationMode: VpnServiceNotificationActionMode,
    ) : VpnServiceStartupDecision

    data class RestoreSystemManagedProxy(val policy: VpnSystemManagedPolicy, val delivery: VpnServiceDelivery) : VpnServiceStartupDecision

    data class Reject(val reason: VpnServiceStartupRejection) : VpnServiceStartupDecision
}

internal fun decideVpnServiceStartup(
    command: VpnServiceCommandExtra,
    delivery: VpnServiceDelivery,
    alwaysOn: VpnAlwaysOnObservation,
): VpnServiceStartupDecision = when (command) {
    VpnServiceCommandExtra.Missing -> when (alwaysOn) {
        VpnAlwaysOnObservation.Enabled -> VpnServiceStartupDecision.RestoreSystemManagedProxy(
            VpnSystemManagedPolicy.AlwaysOnConfirmed,
            delivery,
        )

        VpnAlwaysOnObservation.NotQueryable -> VpnServiceStartupDecision.RestoreSystemManagedProxy(
            VpnSystemManagedPolicy.LegacyUnflaggedStart,
            delivery,
        )

        VpnAlwaysOnObservation.Disabled -> VpnServiceStartupDecision.Reject(
            VpnServiceStartupRejection.AlwaysOnNotEnabled,
        )
    }

    VpnServiceCommandExtra.Malformed -> VpnServiceStartupDecision.Reject(
        VpnServiceStartupRejection.MalformedCommand,
    )

    is VpnServiceCommandExtra.Present -> VpnServiceStartupDecision.ExecuteCommand(
        command = command.command,
        delivery = delivery,
        notificationMode = when (alwaysOn) {
            VpnAlwaysOnObservation.Enabled -> VpnServiceNotificationActionMode.SystemManaged

            VpnAlwaysOnObservation.Disabled,
            VpnAlwaysOnObservation.NotQueryable,
            -> VpnServiceNotificationActionMode.UserManaged
        },
    )
}
