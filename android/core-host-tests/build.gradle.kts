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
                "com/musicfrog/infiltrator/CoreBinaryResolver.kt",
                "com/musicfrog/infiltrator/CoreConfigurationValidationWire.kt",
                "com/musicfrog/infiltrator/CoreProcessState.kt",
                "com/musicfrog/infiltrator/MihomoCoreConfigurationValidator.kt",
                "com/musicfrog/infiltrator/CoreProcessController.kt",
                "com/musicfrog/infiltrator/AndroidOwnerSessionFence.kt",
                "com/musicfrog/infiltrator/AndroidControlIntentWire.kt",
                "com/musicfrog/infiltrator/AndroidCoreSnapshotFence.kt",
                "com/musicfrog/infiltrator/AndroidOwnerDataFence.kt",
                "com/musicfrog/infiltrator/AndroidVpnSnapshotFence.kt",
                "com/musicfrog/infiltrator/VpnOwnerSnapshotWire.kt",
                "com/musicfrog/infiltrator/VpnOwnerStopReceiptWire.kt",
                "com/musicfrog/infiltrator/AndroidVpnAuthorizationFlow.kt",
                "com/musicfrog/infiltrator/OwnerIpcClientPendingLedger.kt",
                "com/musicfrog/infiltrator/VpnOwnerSessionGate.kt",
                "com/musicfrog/infiltrator/VpnCleanupContract.kt",
                "com/musicfrog/infiltrator/VpnOwnerRequestAdmission.kt",
                "com/musicfrog/infiltrator/VpnProcessState.kt",
                "com/musicfrog/infiltrator/VpnRoutingLeaseState.kt",
                "com/musicfrog/infiltrator/VpnServiceBindingState.kt",
                "com/musicfrog/infiltrator/VpnServiceBinding.kt",
                "com/musicfrog/infiltrator/VpnServiceConfiguration.kt",
                "com/musicfrog/infiltrator/VpnConfigurationActivation.kt",
                "com/musicfrog/infiltrator/VpnStartOutcomeWire.kt",
                "com/musicfrog/infiltrator/VpnConfigurationApplyOutcomeWire.kt",
                "com/musicfrog/infiltrator/VpnServiceStartup.kt",
                "com/musicfrog/infiltrator/VpnLifecycleWakeLock.kt",
                "com/musicfrog/infiltrator/AndroidVpnWakeLockHandle.kt",
                "com/musicfrog/infiltrator/MihomoControlServiceFixture.kt",
                "com/musicfrog/infiltrator/IsolatedAuthorizationBindings.kt",
                "infiltrator_android/IsolatedVpnBindings.kt",
            )
        }
        test {
            kotlin.srcDir("../app/src/test/java")
            kotlin.include(
                "com/musicfrog/infiltrator/AndroidCoreProcessRecordStoreTest.kt",
                "com/musicfrog/infiltrator/AndroidControlIntentWireTest.kt",
                "com/musicfrog/infiltrator/AndroidCoreSnapshotFenceTest.kt",
                "com/musicfrog/infiltrator/AndroidOwnerDataFenceTest.kt",
                "com/musicfrog/infiltrator/AndroidOwnerSessionFenceTest.kt",
                "com/musicfrog/infiltrator/AndroidVpnSnapshotFenceTest.kt",
                "com/musicfrog/infiltrator/AndroidVpnAuthorizationFlowTest.kt",
                "com/musicfrog/infiltrator/CoreBinaryResolverTest.kt",
                "com/musicfrog/infiltrator/CoreLaunchSpecWireTest.kt",
                "com/musicfrog/infiltrator/MihomoCoreConfigurationValidatorTest.kt",
                "com/musicfrog/infiltrator/OwnerIpcClientPendingLedgerTypedStateTest.kt",
                "com/musicfrog/infiltrator/OwnerIpcClientPendingLedgerTest.kt",
                "com/musicfrog/infiltrator/VpnCleanupRecoveryTest.kt",
                "com/musicfrog/infiltrator/VpnOwnerRequestAdmissionTest.kt",
                "com/musicfrog/infiltrator/VpnOwnerRequestDeadlineTest.kt",
                "com/musicfrog/infiltrator/VpnProcessStateTest.kt",
                "com/musicfrog/infiltrator/VpnRoutingLeaseStateTest.kt",
                "com/musicfrog/infiltrator/VpnPreparedConfigurationTest.kt",
                "com/musicfrog/infiltrator/VpnConfigurationActivationTest.kt",
                "com/musicfrog/infiltrator/VpnStopOwnerContractTest.kt",
                "com/musicfrog/infiltrator/VpnOwnerSessionGateTest.kt",
                "com/musicfrog/infiltrator/VpnServiceStartupTest.kt",
                "com/musicfrog/infiltrator/VpnLifecycleWakeLockTest.kt",
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
