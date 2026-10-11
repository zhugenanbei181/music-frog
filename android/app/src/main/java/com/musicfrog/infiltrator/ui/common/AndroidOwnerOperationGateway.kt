package com.musicfrog.infiltrator.ui.common

import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidOwnerOperationClientResult
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import infiltrator_android.androidExecuteOwnerOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.withTimeoutOrNull

internal sealed interface AndroidOwnerOperationExchange {
    data class Completed(val reply: AndroidOwnerOperationReply) : AndroidOwnerOperationExchange
    data class Failed(val failure: AppFailure) : AndroidOwnerOperationExchange
    data class Cancelled(val failure: AppFailure) : AndroidOwnerOperationExchange
    data class TimedOut(val failure: AppFailure) : AndroidOwnerOperationExchange
}

internal interface AndroidOwnerOperationGateway {
    suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): AndroidOwnerOperationExchange
}

internal object UniFfiAndroidOwnerOperationGateway : AndroidOwnerOperationGateway {
    override suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): AndroidOwnerOperationExchange = try {
        when (val result = withTimeoutOrNull(timeoutMs) { androidExecuteOwnerOperation(operation) }) {
            null -> AndroidOwnerOperationExchange.TimedOut(
                localizedFailure(FailureTextKey.NotReady).toAppFailure(),
            )
            is AndroidOwnerOperationClientResult.Completed -> AndroidOwnerOperationExchange.Completed(result.reply)
            is AndroidOwnerOperationClientResult.Failed -> result.status.toOwnerOperationExchange()
        }
    } catch (error: CancellationException) {
        throw error
    } catch (error: Exception) {
        AndroidOwnerOperationExchange.Failed(error.toAppFailure())
    }
}

private fun FfiStatus.toOwnerOperationExchange(): AndroidOwnerOperationExchange =
    when (code) {
        FfiErrorCode.CANCELED -> AndroidOwnerOperationExchange.Cancelled(toAppFailure())
        FfiErrorCode.OK -> AndroidOwnerOperationExchange.Failed(
            localizedFailure(FailureTextKey.Internal).toAppFailure(),
        )
        else -> AndroidOwnerOperationExchange.Failed(toAppFailure())
    }
