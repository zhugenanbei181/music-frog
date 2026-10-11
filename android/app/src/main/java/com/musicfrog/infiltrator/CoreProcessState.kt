package com.musicfrog.infiltrator

/** Checked primitive codes exist only at the JNI boundary; callers use this enum. */
internal enum class AndroidCoreProcessState(val wireCode: Int) {
    STOPPED(0),
    RUNNING(1),
    UNAVAILABLE(2),
}
