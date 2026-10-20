package com.musicfrog.infiltrator.ui.connections

import com.musicfrog.infiltrator.ui.common.AndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.CoreIdentityObservation
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import com.musicfrog.infiltrator.ui.common.ScopedProductCommandResult
import infiltrator_android.AndroidCoreLifecycleResult
import infiltrator_android.IpResult
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Test

class ConnectionsRepositoryTest {
    @Test
    fun closeOperationsUseTheObservedScopeAndTypedApplicationCommands() = runBlocking {
        val scope = testScope()
        val owner = FakeAndroidComposeRepository(
            response = """{"Terminal":{"scope":${scope.json},"result":{"Completed":{"request_id":9}}}}""",
        )
        val repository = UniFfiConnectionsRepository(owner)

        assertEquals(ScopedProductCommandResult.Completed("9"), repository.closeConnection(scope, " conn-1 "))
        assertEquals(
            """{"scope":${scope.json},"command":{"Application":{"CloseConnection":{"id":"conn-1"}}}}""",
            owner.lastCommand,
        )
        assertEquals(ScopedProductCommandResult.Completed("9"), repository.closeAll(scope))
        assertEquals(
            """{"scope":${scope.json},"command":{"Application":"CloseAllConnections"}}""",
            owner.lastCommand,
        )
    }

    @Test
    fun aTerminalFromAnotherActivationIsRejected() = runBlocking {
        val scope = testScope()
        val staleScopeJson = scope.json.replace("\"generation\":7", "\"generation\":6")
        val owner = FakeAndroidComposeRepository(
            response = """{"Terminal":{"scope":$staleScopeJson,"result":{"Completed":{"request_id":9}}}}""",
        )

        assertEquals(
            ScopedProductCommandResult.ScopeMismatch,
            UniFfiConnectionsRepository(owner).closeAll(scope),
        )
    }
}

private class FakeAndroidComposeRepository(private val response: String) : AndroidComposeRepository {
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
