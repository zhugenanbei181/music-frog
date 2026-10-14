package com.musicfrog.infiltrator

import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiFailureLocalization
import infiltrator_android.FfiFailureLocalizationKey
import infiltrator_android.FfiStatus

internal fun testFfiStatus(
    code: FfiErrorCode,
    message: String? = null,
): FfiStatus {
    val localizationKey = when (code) {
        FfiErrorCode.OK -> null
        FfiErrorCode.INVALID_STATE -> FfiFailureLocalizationKey.INVALID_STATE
        FfiErrorCode.INVALID_INPUT -> FfiFailureLocalizationKey.INVALID_INPUT
        FfiErrorCode.NOT_READY -> FfiFailureLocalizationKey.NOT_READY
        FfiErrorCode.NOT_SUPPORTED -> FfiFailureLocalizationKey.UNSUPPORTED
        FfiErrorCode.IO -> FfiFailureLocalizationKey.STORAGE
        FfiErrorCode.NETWORK -> FfiFailureLocalizationKey.NETWORK
        FfiErrorCode.AUTH -> FfiFailureLocalizationKey.AUTHENTICATION
        FfiErrorCode.SYNC -> FfiFailureLocalizationKey.SYNC
        FfiErrorCode.CONFIG -> FfiFailureLocalizationKey.CONFIGURATION
        FfiErrorCode.CANCELED -> FfiFailureLocalizationKey.CANCELED
        FfiErrorCode.UNKNOWN -> FfiFailureLocalizationKey.INTERNAL
    }
    val localization = localizationKey?.let { key ->
        FfiFailureLocalization(key = key, line = null, column = null)
    }
    return FfiStatus(code = code, message = message, localization = localization)
}
