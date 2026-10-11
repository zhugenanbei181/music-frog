package com.musicfrog.infiltrator

import infiltrator_android.AndroidControlIntent

internal object AndroidControlIntentWire {
    const val EXTRA = "com.musicfrog.infiltrator.CONTROL_INTENT"
}

/** Explicit Android/JNI wire mapping mirrored by the Rust contract enum. */
internal fun AndroidControlIntent.toWireCode(): Int =
    when (this) {
        AndroidControlIntent.START_PROXY -> 0
        AndroidControlIntent.STOP_PROXY -> 1
        AndroidControlIntent.RESUME_KERNEL -> 2
        AndroidControlIntent.PAUSE_KERNEL -> 3
        AndroidControlIntent.RESTART_KERNEL -> 4
    }

internal fun androidControlIntentFromWireCode(code: Int): AndroidControlIntent? =
    when (code) {
        0 -> AndroidControlIntent.START_PROXY
        1 -> AndroidControlIntent.STOP_PROXY
        2 -> AndroidControlIntent.RESUME_KERNEL
        3 -> AndroidControlIntent.PAUSE_KERNEL
        4 -> AndroidControlIntent.RESTART_KERNEL
        else -> null
    }
