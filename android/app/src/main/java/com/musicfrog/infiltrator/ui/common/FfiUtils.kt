package com.musicfrog.infiltrator.ui.common

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.withTimeoutOrNull

const val DEFAULT_FFI_TIMEOUT_MS = 8000L
const val LONG_FFI_TIMEOUT_MS = 20000L

internal sealed interface CallResult<out T> {
    data class Success<out T>(val value: T) : CallResult<T>
    data class Failure(val failure: AppFailure) : CallResult<Nothing>
}

internal inline fun <T> CallResult<T>.getOrElse(onFailure: (AppFailure) -> Nothing): T = when (this) {
    is CallResult.Success -> value
    is CallResult.Failure -> onFailure(failure)
}

private data class TimedCall<out T>(val value: T)

/**
 * Run one suspend FFI operation with a typed timeout and failure result.
 * Parent cancellation is control flow and must remain visible to its caller.
 */
internal suspend fun <T> runCall(timeoutMs: Long = DEFAULT_FFI_TIMEOUT_MS, block: suspend () -> T): CallResult<T> = try {
    val timed = withTimeoutOrNull(timeoutMs) { TimedCall(block()) }
    if (timed == null) {
        CallResult.Failure(
            AppFailure(
                code = AppFailureCode.NotReady,
                localization = FailureTextKey.NotReady,
            ),
        )
    } else {
        CallResult.Success(timed.value)
    }
} catch (error: TimeoutCancellationException) {
    // `withTimeoutOrNull` consumes only its own timeout. A nested timeout is
    // still cancellation from the parent operation and is rethrown here.
    throw error
} catch (error: CancellationException) {
    throw error
} catch (error: Exception) {
    CallResult.Failure(error.toAppFailure())
}

internal fun Throwable.toAppFailure(): AppFailure = AppFailure(
    code = AppFailureCode.Internal,
    localization = FailureTextKey.Internal,
    diagnostic = message?.takeIf(String::isNotBlank),
)

internal fun Throwable.toLocalizedFailure(): LocalizedFailure = localizedFailure(
    FailureTextKey.Internal,
    message,
)
