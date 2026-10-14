package com.musicfrog.infiltrator.ui.logs

import com.musicfrog.infiltrator.ui.common.AndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.CoreIdentityObservation
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import infiltrator_android.AndroidCoreLifecycleResult
import infiltrator_android.IpResult
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Test

class LogsRepositoryTest {
    @Test
    fun clearUsesScopedSharedProductCommandAndAcceptsMatchingCompletion() = runBlocking {
        val scope = testScope()
        val owner = FakeAndroidComposeRepository(
            response = """{"Terminal":{"scope":${scope.json},"result":{"Completed":{"request_id":9}}}}""",
        )

        val result = UniFfiLogsRepository(owner).clearLogs(scope)

        assertEquals(LogsClearResult.Accepted, result)
        assertEquals(
            """{"scope":${scope.json},"command":{"Application":"ClearLogs"}}""",
            owner.lastCommand,
        )
    }

    @Test
    fun clearRejectsACompletionFromAnotherCoreActivation() = runBlocking {
        val scope = testScope()
        val owner = FakeAndroidComposeRepository(
            response = """{"Terminal":{"scope":${scope.json.replace("\"generation\":7", "\"generation\":8")},"result":{"Completed":{"request_id":9}}}}""",
        )

        assertEquals(
            LogsClearResult.Failed(logsNotReadyFailure()),
            UniFfiLogsRepository(owner).clearLogs(scope),
        )
    }
}

private class FakeAndroidComposeRepository(
    private val response: String,
) : AndroidComposeRepository {
    var lastCommand: String? = null

    override suspend fun readOwnerSurface(): OwnerSurfaceRead = error("unused")
    override suspend fun executeProductCommand(commandJson: String): String {
        lastCommand = commandJson
        return response
    }
    override suspend fun readCoreLifecycle(): AndroidCoreLifecycleResult = error("unused")
    override suspend fun checkPublicIp(): IpResult = error("unused")
}

private fun testScope() = CoreScopeObservation(
    identity = CoreIdentityObservation("mihomo", "android-vpn", "1.19.12"),
    generation = "7",
    sessionToken = "8",
    json = """{"identity":{"engine":"mihomo","instance":"android-vpn","version":"1.19.12"},"generation":7,"session_token":8}""",
)
