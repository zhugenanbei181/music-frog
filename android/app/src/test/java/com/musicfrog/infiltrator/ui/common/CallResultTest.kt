package com.musicfrog.infiltrator.ui.common

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test

class CallResultTest {
    @Test
    fun successfulCallHasOnlyTheAcceptedValueVariant() = runBlocking {
        assertEquals(CallResult.Success("ready"), runCall { "ready" })
    }

    @Test
    fun thrownFailureProjectsTypedCodeAndSharedLocalization() = runBlocking {
        assertEquals(
            CallResult.Failure(
                AppFailure(
                    code = AppFailureCode.Internal,
                    localization = FailureTextKey.Internal,
                    diagnostic = "opaque backend detail",
                ),
            ),
            runCall { throw IllegalStateException("opaque backend detail") },
        )
    }

    @Test
    fun onlyTheCallTimeoutBecomesNotReadyFailure() = runBlocking {
        val result = runCall(timeoutMs = 1) {
            delay(50)
            "late"
        }

        assertEquals(
            CallResult.Failure(
                AppFailure(
                    code = AppFailureCode.NotReady,
                    localization = FailureTextKey.NotReady,
                ),
            ),
            result,
        )
    }

    @Test
    fun parentCancellationIsRethrownWithoutLocalization() = runBlocking {
        val parentCancellation = CancellationException("caller canceled")
        var actual: CancellationException? = null

        try {
            runCall { throw parentCancellation }
        } catch (error: CancellationException) {
            actual = error
        }

        assertSame(parentCancellation, actual)
    }
}
