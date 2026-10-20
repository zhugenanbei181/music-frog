package com.musicfrog.infiltrator

import android.os.PowerManager

internal class AndroidVpnWakeLockHandle(private val wakeLock: PowerManager.WakeLock) : VpnWakeLockHandle {
    override fun acquire(timeoutMillis: Long): VpnWakeLockPlatformAcquire = try {
        wakeLock.acquire(timeoutMillis)
        VpnWakeLockPlatformAcquire.Acquired
    } catch (error: RuntimeException) {
        VpnWakeLockPlatformAcquire.Unavailable(
            error.message ?: "Android partial WakeLock could not be acquired",
        )
    }

    override fun release(): VpnWakeLockPlatformRelease = try {
        if (wakeLock.isHeld) {
            wakeLock.release()
            VpnWakeLockPlatformRelease.Released
        } else {
            VpnWakeLockPlatformRelease.Expired
        }
    } catch (error: RuntimeException) {
        VpnWakeLockPlatformRelease.Failed(
            error.message ?: "Android partial WakeLock release failed",
        )
    }
}
