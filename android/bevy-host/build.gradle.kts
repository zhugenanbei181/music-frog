import java.util.Properties
import java.io.File

// BANDROID-003/004 — Rust Bevy driver packaged only by the Bevy product flavor.

plugins {
    id("com.android.library")
}

android {
    namespace = "com.musicfrog.infiltrator.bevyhost"
    compileSdk = 37
    // BANDROID-020: keep the toolchain identical to :app.
    ndkVersion = "29.0.14206865"

    defaultConfig {
        minSdk = 29
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.directories.add("build/rustJniLibs/android")
        }
    }
}

val localProperties = Properties()
val localPropertiesFile = rootProject.file("local.properties")
if (localPropertiesFile.isFile) {
    localPropertiesFile.inputStream().use(localProperties::load)
}
val rustNdkHome = sequenceOf(
    System.getenv("ANDROID_NDK_HOME"),
    System.getenv("ANDROID_NDK_ROOT"),
    localProperties.getProperty("ndk.dir"),
).firstOrNull { !it.isNullOrBlank() }
    ?: error("Set ANDROID_NDK_HOME, ANDROID_NDK_ROOT, or ndk.dir in local.properties")
val ndkHostTag = when {
    System.getProperty("os.name").startsWith("Mac", ignoreCase = true) ->
        if (System.getProperty("os.arch") == "aarch64") "darwin-aarch64" else "darwin-x86_64"
    System.getProperty("os.name").startsWith("Windows", ignoreCase = true) -> "windows-x86_64"
    else -> "linux-x86_64"
}
val ndkToolchainBin = File(rustNdkHome, "toolchains/llvm/prebuilt/$ndkHostTag/bin")
check(ndkToolchainBin.isDirectory) {
    "Android NDK LLVM toolchain directory is missing: $ndkToolchainBin"
}

// Build the Rust driver for each ABI that the Bevy flavor ships.
val rustJniLibsDir = layout.buildDirectory.dir("rustJniLibs/android")
val driverDir = file("driver")
val rustTargets = listOf(
    "arm64-v8a" to "aarch64-linux-android",
    "x86_64" to "x86_64-linux-android",
)

val driverCopies = rustTargets.map { (androidAbi, rustTarget) ->
    val taskSuffix = androidAbi.split('-').joinToString("") { segment ->
        segment.replaceFirstChar { it.uppercase() }
    }
    val buildDriver = tasks.register<Exec>("buildRustDriver$taskSuffix") {
        workingDir = driverDir
        environment("ANDROID_NDK_HOME", rustNdkHome)
        val targetClang = ndkToolchainBin.resolve("${rustTarget}21-clang").absolutePath
        val targetAr = ndkToolchainBin.resolve("llvm-ar").absolutePath
        environment("CC_$rustTarget", targetClang)
        environment("CC_${rustTarget.replace('-', '_')}", targetClang)
        environment("AR_$rustTarget", targetAr)
        environment("AR_${rustTarget.replace('-', '_')}", targetAr)
        environment(
            "PATH",
            "${ndkToolchainBin.absolutePath}${File.pathSeparator}${System.getenv("PATH").orEmpty()}",
        )
        commandLine(
            "cargo",
            "build",
            "--release",
            "--locked",
            "--target",
            rustTarget,
        )
    }
    tasks.register<Copy>("copyRustDriver$taskSuffix") {
        dependsOn(buildDriver)
        from(
            driverDir.resolve(
                "target/$rustTarget/release/libinfiltrator_bevy_android.so",
            ),
        )
        into(rustJniLibsDir.map { it.dir(androidAbi) })
    }
}

tasks.named("preBuild") {
    dependsOn(driverCopies)
}
