import java.util.Properties

plugins {
    id("org.jetbrains.kotlin.jvm")
}

val localProperties = Properties()
rootProject.file("local.properties").takeIf { it.isFile }?.inputStream()?.use { input ->
    localProperties.load(input)
}
val androidSdkRoot = providers.environmentVariable("ANDROID_HOME").orNull
    ?: providers.environmentVariable("ANDROID_SDK_ROOT").orNull
    ?: localProperties.getProperty("sdk.dir")
    ?: error("Set ANDROID_HOME/ANDROID_SDK_ROOT or sdk.dir in android/local.properties")
val androidApiJar = file("$androidSdkRoot/platforms/android-37.0/android.jar")

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
    sourceSets {
        main {
            kotlin.srcDir("../app/src/main/java")
            kotlin.include(
                "com/musicfrog/infiltrator/AndroidProcessRole.kt",
                "com/musicfrog/infiltrator/CoreLaunchSpecWire.kt",
                "com/musicfrog/infiltrator/CoreConfigurationValidationWire.kt",
                "com/musicfrog/infiltrator/CoreProcessState.kt",
                "com/musicfrog/infiltrator/MihomoCoreConfigurationValidator.kt",
                "com/musicfrog/infiltrator/MihomoProcessController.kt",
                "com/musicfrog/infiltrator/AndroidOwnerSessionFence.kt",
                "com/musicfrog/infiltrator/OwnerIpcClientPendingLedger.kt",
                "com/musicfrog/infiltrator/VpnOwnerSessionGate.kt",
            )
        }
        test {
            kotlin.srcDir("../app/src/test/java")
            kotlin.include(
                "com/musicfrog/infiltrator/AndroidCoreProcessRecordStoreTest.kt",
                "com/musicfrog/infiltrator/CoreLaunchSpecWireTest.kt",
                "com/musicfrog/infiltrator/MihomoCoreConfigurationValidatorTest.kt",
                "com/musicfrog/infiltrator/OwnerIpcClientPendingLedgerTypedStateTest.kt",
                "com/musicfrog/infiltrator/OwnerIpcClientPendingLedgerTest.kt",
                "com/musicfrog/infiltrator/VpnOwnerSessionGateTest.kt",
            )
        }
    }
}

dependencies {
    compileOnly(files(androidApiJar))
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}

tasks.withType<org.gradle.api.tasks.compile.JavaCompile>().configureEach {
    sourceCompatibility = "17"
    targetCompatibility = "17"
}

tasks.withType<org.gradle.api.tasks.testing.Test>().configureEach {
    classpath += files(androidApiJar)
}
