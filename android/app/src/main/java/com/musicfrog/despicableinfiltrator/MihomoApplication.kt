package com.musicfrog.despicableinfiltrator

import android.app.Application
import android.util.Log
import infiltrator_android.bridgeReady
import infiltrator_android.bridgeShutdown

class MihomoApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        Log.i("MihomoApp", "Initializing application...")
        
        try {
            System.loadLibrary("infiltrator_android")
            Log.i("MihomoApp", "Native library loaded.")
        } catch (e: UnsatisfiedLinkError) {
            Log.e("MihomoApp", "Failed to load native library", e)
        }
    }

    override fun onTerminate() {
        super.onTerminate()
        bridgeShutdown()
    }
}
