package com.musicfrog.despicableinfiltrator.ui.common

import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.withTimeout

const val DEFAULT_FFI_TIMEOUT_MS = 8000L
const val LONG_FFI_TIMEOUT_MS = 20000L

data class FfiCallResult<T>(
    val value: T?,
    val error: String?
)

suspend fun <T> runFfiCall(
    timeoutMs: Long = DEFAULT_FFI_TIMEOUT_MS,
    block: suspend () -> T
): FfiCallResult<T> {
    return try {
        val value = withTimeout(timeoutMs) { block() }
        FfiCallResult(value, null)
    } catch (err: TimeoutCancellationException) {
        FfiCallResult(null, "Request timed out")
    } catch (err: Exception) {
        FfiCallResult(null, err.message ?: "Unexpected error")
    }
}

fun FfiStatus.userMessage(fallback: String): String {
    val detail = message?.trim().orEmpty()
    if (detail.isNotEmpty()) {
        return detail
    }
    val codeMessage = when (code) {
        FfiErrorCode.OK -> "OK"
        FfiErrorCode.INVALID_STATE -> "Invalid state"
        FfiErrorCode.INVALID_INPUT -> "Invalid input"
        FfiErrorCode.NOT_READY -> "Not ready"
        FfiErrorCode.NOT_SUPPORTED -> "Not supported"
        FfiErrorCode.IO -> "I/O error"
        FfiErrorCode.NETWORK -> "Network error"
        FfiErrorCode.UNKNOWN -> "Unknown error"
    }
    return if (fallback.isBlank()) codeMessage else "$fallback: $codeMessage"
}

fun emptyMessage(label: String): String {
    return "No $label available"
}
