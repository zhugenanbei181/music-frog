package com.musicfrog.infiltrator.ui.settings.rules

import com.musicfrog.infiltrator.testFfiStatus
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationExchange
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationGateway
import infiltrator_android.AndroidOwnerDataDomain
import infiltrator_android.AndroidOwnerDataSnapshot
import infiltrator_android.AndroidOwnerDataSnapshotData
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidOwnerOperationContext
import infiltrator_android.AndroidOwnerOperationProtocolVersion
import infiltrator_android.AndroidOwnerOperationReadback
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.AndroidRuleEntry
import infiltrator_android.AndroidRulesResult
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RulesRepositoryTest {
    @Test
    fun readsUseTheOwnerDataRevisionAndTypedSnapshot() = runBlocking {
        val entry = AndroidRuleEntry("DOMAIN,example.org,DIRECT", true)
        val operation = AndroidOwnerOperation.RulesList

        val result = repository { completed(reply(operation, listOf(entry), revision = 10uL)) }
            .execute(operation, 1_000L)

        val observed = (result as RulesRepositoryResult.Completed).observation as RulesRepositoryObservation.Rules
        assertEquals(listOf(entry), observed.snapshot.entries)
        assertEquals(RulesRevision("owner-session", 7uL, 10uL), observed.snapshot.revision)
    }

    @Test
    fun successfulWritesRequireRevisionAdvanceAndExactReadback() = runBlocking {
        val entry = AndroidRuleEntry("DOMAIN,example.org,DIRECT", true)
        val operation = AndroidOwnerOperation.RulesSave(listOf(entry))

        val confirmed = repository { completed(reply(operation, listOf(entry), revision = 11uL)) }
            .execute(operation, 1_000L)
        val stale = repository { completed(reply(operation, listOf(entry), revision = 10uL)) }
            .execute(operation, 1_000L)
        val mismatched = repository {
            completed(reply(operation, listOf(AndroidRuleEntry("DOMAIN,other.org,REJECT", true)), revision = 11uL))
        }.execute(operation, 1_000L)

        assertTrue(confirmed is RulesRepositoryResult.Completed)
        assertEquals(
            11uL,
            (((confirmed as RulesRepositoryResult.Completed).observation as RulesRepositoryObservation.Rules)
                .snapshot.revision.dataRevision),
        )
        assertTrue(stale is RulesRepositoryResult.Unconfirmed)
        assertTrue(mismatched is RulesRepositoryResult.Unconfirmed)
    }

    private fun repository(
        handle: suspend (AndroidOwnerOperation) -> AndroidOwnerOperationExchange,
    ) = UniFfiRulesRepository(object : AndroidOwnerOperationGateway {
        override suspend fun execute(
            operation: AndroidOwnerOperation,
            timeoutMs: Long,
        ): AndroidOwnerOperationExchange = handle(operation)
    })

    private fun completed(reply: AndroidOwnerOperationReply) = AndroidOwnerOperationExchange.Completed(reply)

    private fun reply(
        operation: AndroidOwnerOperation,
        rules: List<AndroidRuleEntry>,
        revision: ULong,
    ): AndroidOwnerOperationReply {
        val snapshotResult = AndroidRulesResult(testFfiStatus(FfiErrorCode.OK), rules)
        return AndroidOwnerOperationReply(
            protocol = AndroidOwnerOperationProtocolVersion.V6,
            context = AndroidOwnerOperationContext(
                clientSessionId = "client-session",
                expectedOwnerSessionId = "owner-session",
                ownerSessionId = "owner-session",
                expectedCoreGeneration = 7uL,
                expectedDataRevision = 10uL,
            ),
            observedCoreGeneration = 7uL,
            dataRevision = revision,
            dataDomain = AndroidOwnerDataDomain.RULES,
            snapshot = AndroidOwnerDataSnapshot(
                domain = AndroidOwnerDataDomain.RULES,
                data = AndroidOwnerDataSnapshotData.Rules(snapshotResult),
            ),
            readback = AndroidOwnerOperationReadback.Rules(snapshotResult),
        ).also { require(operation is AndroidOwnerOperation.RulesList || operation is AndroidOwnerOperation.RulesSave) }
    }
}
