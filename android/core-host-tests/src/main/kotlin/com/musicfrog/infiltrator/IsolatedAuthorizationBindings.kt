package com.musicfrog.infiltrator

/** Compile-only shapes for the platform permission adapter's pure host flow tests. */
internal enum class NotificationPermissionStatus {
    NotRequired,
    Granted,
    NotGranted,
}

internal enum class NotificationPermissionPromptResult {
    Granted,
    NotGranted,
}
