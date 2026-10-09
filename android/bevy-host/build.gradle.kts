// BANDROID-003/004 — Bevy Android product host packaging module.
//
// STATUS: isolated / unverified. This module is intentionally NOT listed in
// android/settings.gradle.kts yet; enabling it requires a verified JDK + Gradle
// + NDK + Rust Android toolchain. See ./README.md for the wiring checklist.
//
// Purpose: build the native driver (android/bevy-host/driver) against
// `infiltrator-android` with the `bevy-host` feature and expose the resulting
// `libinfiltrator_bevy_android.so` so `:app` can package it next to
// BevyHostActivity. The Compose product, the VPN `<service>` and the boot
// receiver all stay owned by `:app`.

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.musicfrog.infiltrator.bevyhost"
    compileSdk = 36
    // BANDROID-020: keep the toolchain identical to :app.
    ndkVersion = "29.0.14206865"

    defaultConfig {
        minSdk = 29
        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("build/rustJniLibs/android")
        }
    }
}

// Build the Rust driver and stage the .so where the Android plugin picks it up.
// `INFILTRATOR_ANDROID_TARGET` defaults to the shipped aarch64 ABI.
val rustJniLibsDir = layout.buildDirectory.dir("rustJniLibs/android")
val driverDir = file("driver")

val buildRustDriver by tasks.registering(Exec::class) {
    workingDir = driverDir
    environment("ANDROID_NDK_HOME", android.ndkDirectory.absolutePath)
    commandLine(
        "cargo",
        "build",
        "--release",
        "--target",
        "aarch64-linux-android",
        "--features",
        "bevy-host",
    )
}

val copyRustDriver by tasks.registering(Copy::class) {
    dependsOn(buildRustDriver)
    from(driverDir.resolve("target/aarch64-linux-android/release/libinfiltrator_bevy_android.so"))
    into(rustJniLibsDir.map { it.dir("arm64-v8a") })
}

tasks.named("preBuild") {
    dependsOn(copyRustDriver)
}
