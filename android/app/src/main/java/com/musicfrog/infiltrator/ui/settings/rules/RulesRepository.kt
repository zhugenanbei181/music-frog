package com.musicfrog.infiltrator.ui.settings.rules

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationExchange
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationGateway
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.UniFfiAndroidOwnerOperationGateway
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidOwnerDataDomain
import infiltrator_android.AndroidOwnerDataSnapshotData
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidOwnerOperationReadback
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.AndroidRuleEntry
import infiltrator_android.AndroidRuleProviderEntry
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus

internal sealed interface RulesRepositoryObservation {
    data class Rules(val snapshot: RulesSnapshot) : RulesRepositoryObservation
    data class Providers(val snapshot: RuleProvidersSnapshot) : RulesRepositoryObservation
}

internal sealed interface RulesRepositoryResult {
    data class Completed(val observation: RulesRepositoryObservation) : RulesRepositoryResult
    data class Failed(
        val failure: AppFailure,
        val latest: RulesRepositoryObservation?,
    ) : RulesRepositoryResult
    data class Cancelled(
        val failure: AppFailure,
        val latest: RulesRepositoryObservation?,
    ) : RulesRepositoryResult
    data class TimedOut(val failure: AppFailure) : RulesRepositoryResult
    data class Unconfirmed(
        val failure: AppFailure,
        val latest: RulesRepositoryObservation?,
    ) : RulesRepositoryResult
}

internal interface RulesRepository {
    suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): RulesRepositoryResult
}

internal object UniFfiRulesRepository : RulesRepository {
    private val gateway: AndroidOwnerOperationGateway = UniFfiAndroidOwnerOperationGateway

    override suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): RulesRepositoryResult = when (val result = gateway.execute(operation, timeoutMs)) {
        is AndroidOwnerOperationExchange.Completed -> result.reply.toRulesRepositoryResult(operation)
        is AndroidOwnerOperationExchange.Failed -> RulesRepositoryResult.Failed(result.failure, null)
        is AndroidOwnerOperationExchange.Cancelled -> RulesRepositoryResult.Cancelled(result.failure, null)
        is AndroidOwnerOperationExchange.TimedOut -> RulesRepositoryResult.TimedOut(result.failure)
    }
}

private fun AndroidOwnerOperationReply.toRulesRepositoryResult(
    operation: AndroidOwnerOperation,
): RulesRepositoryResult {
    val purpose = operation.rulesRequestKind()
    val domain = purpose.domain
    val latest = latestObservation(domain)
    val expectedDomain = when (domain) {
        RulesDomain.Rules -> AndroidOwnerDataDomain.RULES
        RulesDomain.Providers -> AndroidOwnerDataDomain.RULE_PROVIDERS
        RulesDomain.Unsupported -> return unconfirmedRules(latest)
    }
    if (dataDomain != expectedDomain || snapshot?.domain != expectedDomain ||
        context.clientSessionId.isBlank() || context.ownerSessionId.isBlank() ||
        context.expectedOwnerSessionId != context.ownerSessionId ||
        context.expectedCoreGeneration != observedCoreGeneration
    ) {
        return unconfirmedRules(latest)
    }

    val status = when (val value = readbackStatus(readback, purpose)) {
        is RulesReadbackStatus.Observed -> value.status
        RulesReadbackStatus.Mismatch -> return unconfirmedRules(latest)
    }
    val expectedRevision = when (purpose.revisionAdvance(status.code.toOwnerResultStatus())) {
        RulesRevisionAdvance.None -> context.expectedDataRevision
        RulesRevisionAdvance.One -> context.expectedDataRevision
            .takeIf { it < ULong.MAX_VALUE }
            ?.plus(1u)
            ?: return unconfirmedRules(latest)
    }
    if (dataRevision != expectedRevision) return unconfirmedRules(latest)

    return when (status.code.toOwnerResultStatus()) {
        OwnerResultStatus.Succeeded -> {
            val observation = latest ?: return unconfirmedRules(null)
            when (readbackSnapshotConsistency(readback, snapshot?.data)) {
                ReadbackSnapshotConsistency.Mismatch -> return unconfirmedRules(observation)
                ReadbackSnapshotConsistency.Matches -> Unit
            }
            when (requestedWriteConfirmation(purpose, operation, observation)) {
                RequestedWriteConfirmation.Confirmed,
                RequestedWriteConfirmation.NotRequired -> RulesRepositoryResult.Completed(observation)
                RequestedWriteConfirmation.Mismatch -> unconfirmedRules(observation)
            }
        }
        OwnerResultStatus.Cancelled -> RulesRepositoryResult.Cancelled(status.toAppFailure(), latest)
        OwnerResultStatus.Failed -> RulesRepositoryResult.Failed(status.toAppFailure(), latest)
    }
}

private fun AndroidOwnerOperation.rulesRequestKind(): RulesRequestKind = when (this) {
    AndroidOwnerOperation.RulesList -> RulesRequestKind.ReadRules
    is AndroidOwnerOperation.RulesSave -> RulesRequestKind.WriteRules
    AndroidOwnerOperation.RuleProviders -> RulesRequestKind.ReadProviders
    is AndroidOwnerOperation.RuleProvidersSave -> RulesRequestKind.WriteProviders
    else -> RulesRequestKind.Unsupported
}

private fun RulesRequestKind.revisionAdvance(
    status: OwnerResultStatus,
): RulesRevisionAdvance = when (this) {
    RulesRequestKind.ReadRules,
    RulesRequestKind.ReadProviders,
    RulesRequestKind.Unsupported -> RulesRevisionAdvance.None
    RulesRequestKind.WriteRules,
    RulesRequestKind.WriteProviders -> when (status) {
        OwnerResultStatus.Succeeded -> RulesRevisionAdvance.One
        OwnerResultStatus.Cancelled,
        OwnerResultStatus.Failed -> RulesRevisionAdvance.None
    }
}

private fun readbackStatus(
    readback: AndroidOwnerOperationReadback,
    purpose: RulesRequestKind,
): RulesReadbackStatus = when (purpose) {
    RulesRequestKind.ReadRules,
    RulesRequestKind.WriteRules -> when (readback) {
        is AndroidOwnerOperationReadback.Rules -> RulesReadbackStatus.Observed(readback.v1.status)
        else -> RulesReadbackStatus.Mismatch
    }
    RulesRequestKind.ReadProviders,
    RulesRequestKind.WriteProviders -> when (readback) {
        is AndroidOwnerOperationReadback.RuleProviders -> RulesReadbackStatus.Observed(readback.v1.status)
        else -> RulesReadbackStatus.Mismatch
    }
    RulesRequestKind.Unsupported -> RulesReadbackStatus.Mismatch
}

private fun AndroidOwnerOperationReply.latestObservation(
    domain: RulesDomain,
): RulesRepositoryObservation? {
    val ownerDataDomain = domain.toOwnerDataDomain() ?: return null
    if (snapshot?.domain != ownerDataDomain || dataDomain != ownerDataDomain) return null
    val revision = RulesRevision(
        ownerSessionId = context.ownerSessionId,
        coreGeneration = observedCoreGeneration,
        dataRevision = dataRevision,
    )
    return when (val value = snapshot?.data) {
        is AndroidOwnerDataSnapshotData.Rules -> when (domain) {
            RulesDomain.Rules -> when (value.v1.status.code.toOwnerResultStatus()) {
                OwnerResultStatus.Succeeded -> RulesRepositoryObservation.Rules(
                    RulesSnapshot(value.v1.rules, revision),
                )
                OwnerResultStatus.Cancelled,
                OwnerResultStatus.Failed -> null
            }
            RulesDomain.Providers,
            RulesDomain.Unsupported -> null
        }
        is AndroidOwnerDataSnapshotData.RuleProviders -> when (domain) {
            RulesDomain.Providers -> when (value.v1.status.code.toOwnerResultStatus()) {
                OwnerResultStatus.Succeeded -> {
                    val json = runCatching { ruleProviderEntriesToJson(value.v1.providers) }.getOrNull()
                        ?: return null
                    RulesRepositoryObservation.Providers(
                        RuleProvidersSnapshot(value.v1.providers, json, revision),
                    )
                }
                OwnerResultStatus.Cancelled,
                OwnerResultStatus.Failed -> null
            }
            RulesDomain.Rules,
            RulesDomain.Unsupported -> null
        }
        else -> null
    }
}

private fun readbackSnapshotConsistency(
    readback: AndroidOwnerOperationReadback,
    snapshot: AndroidOwnerDataSnapshotData?,
): ReadbackSnapshotConsistency = when (readback) {
    is AndroidOwnerOperationReadback.Rules -> when (snapshot) {
        is AndroidOwnerDataSnapshotData.Rules -> when (readback.v1 == snapshot.v1) {
            true -> ReadbackSnapshotConsistency.Matches
            false -> ReadbackSnapshotConsistency.Mismatch
        }
        else -> ReadbackSnapshotConsistency.Mismatch
    }
    is AndroidOwnerOperationReadback.RuleProviders -> when (snapshot) {
        is AndroidOwnerDataSnapshotData.RuleProviders -> when (readback.v1 == snapshot.v1) {
            true -> ReadbackSnapshotConsistency.Matches
            false -> ReadbackSnapshotConsistency.Mismatch
        }
        else -> ReadbackSnapshotConsistency.Mismatch
    }
    else -> ReadbackSnapshotConsistency.Mismatch
}

private fun requestedWriteConfirmation(
    purpose: RulesRequestKind,
    operation: AndroidOwnerOperation,
    observation: RulesRepositoryObservation,
): RequestedWriteConfirmation = when (purpose) {
    RulesRequestKind.ReadRules,
    RulesRequestKind.ReadProviders -> RequestedWriteConfirmation.NotRequired
    RulesRequestKind.WriteRules -> when {
        operation !is AndroidOwnerOperation.RulesSave -> RequestedWriteConfirmation.Mismatch
        observation !is RulesRepositoryObservation.Rules -> RequestedWriteConfirmation.Mismatch
        operation.rules == observation.snapshot.entries -> RequestedWriteConfirmation.Confirmed
        else -> RequestedWriteConfirmation.Mismatch
    }
    RulesRequestKind.WriteProviders -> when {
        operation !is AndroidOwnerOperation.RuleProvidersSave -> RequestedWriteConfirmation.Mismatch
        observation !is RulesRepositoryObservation.Providers -> RequestedWriteConfirmation.Mismatch
        operation.providers == observation.snapshot.entries -> RequestedWriteConfirmation.Confirmed
        else -> RequestedWriteConfirmation.Mismatch
    }
    RulesRequestKind.Unsupported -> RequestedWriteConfirmation.Mismatch
}

private fun RulesDomain.toOwnerDataDomain(): AndroidOwnerDataDomain? = when (this) {
    RulesDomain.Rules -> AndroidOwnerDataDomain.RULES
    RulesDomain.Providers -> AndroidOwnerDataDomain.RULE_PROVIDERS
    RulesDomain.Unsupported -> null
}

private fun FfiErrorCode.toOwnerResultStatus(): OwnerResultStatus = when (this) {
    FfiErrorCode.OK -> OwnerResultStatus.Succeeded
    FfiErrorCode.CANCELED -> OwnerResultStatus.Cancelled
    else -> OwnerResultStatus.Failed
}

private enum class OwnerResultStatus {
    Succeeded,
    Cancelled,
    Failed,
}

private sealed interface RulesReadbackStatus {
    data class Observed(val status: FfiStatus) : RulesReadbackStatus
    data object Mismatch : RulesReadbackStatus
}

private enum class RulesRevisionAdvance {
    None,
    One,
}

private enum class ReadbackSnapshotConsistency {
    Matches,
    Mismatch,
}

private enum class RequestedWriteConfirmation {
    Confirmed,
    NotRequired,
    Mismatch,
}

private fun notReadyFailure() = localizedFailure(FailureTextKey.NotReady).toAppFailure()

private fun unconfirmedRules(
    latest: RulesRepositoryObservation?,
) = RulesRepositoryResult.Unconfirmed(notReadyFailure(), latest)
