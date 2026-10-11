import java.util.Properties
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction

abstract class GenerateProductIcon : DefaultTask() {
    @get:InputFile
    abstract val sourceIcon: RegularFileProperty

    @get:Input
    abstract val resourceName: Property<String>

    @get:OutputDirectory
    abstract val outputDirectory: DirectoryProperty

    @TaskAction
    fun generate() {
        val target = outputDirectory.get().file("drawable/${resourceName.get()}.png").asFile
        target.parentFile.mkdirs()
        sourceIcon.get().asFile.copyTo(target, overwrite = true)
    }
}

val productIdentityFile = rootProject.file("../product-identity.properties")
check(productIdentityFile.isFile) {
    "Android product identity file is missing: $productIdentityFile"
}
val productIdentity = Properties().apply {
    productIdentityFile.inputStream().use { load(it) }
}
fun identityValue(key: String): String =
    productIdentity.getProperty(key)?.takeIf { it.isNotBlank() }
        ?: error("Missing Android product identity key '$key' in $productIdentityFile")

fun generatedIdentityValue(key: String): String {
    val output = providers.exec {
        commandLine(
            "python3",
            rootProject.file("../scripts/product_identity.py").absolutePath,
            "--get",
            key,
        )
    }
    val result = output.result.get()
    check(result.exitValue == 0) {
        "Cannot read product identity '$key': ${output.standardError.asText.get()}"
    }
    return output.standardOutput.asText.get().trim()
}

val productVersionName = generatedIdentityValue("version.name")
val composeVersionCode = generatedIdentityValue("compose.version_code").toInt()
val bevyVersionCode = generatedIdentityValue("bevy.version_code").toInt()
val slintVersionCode = generatedIdentityValue("slint.version_code").toInt()
val composeApplicationId = identityValue("compose.application_id")
val bevyApplicationId = identityValue("bevy.application_id")
val slintApplicationId = identityValue("slint.application_id")
val androidNamespace = identityValue("android.namespace")
val composeDisplayName = identityValue("compose.display_name")
val bevyDisplayName = identityValue("bevy.display_name")
val slintDisplayName = identityValue("slint.display_name")
val composeIconResource = identityValue("compose.icon_resource")
val bevyIconResource = identityValue("bevy.icon_resource")
val slintIconResource = identityValue("slint.icon_resource")
val composeIconAsset = identityValue("compose.icon_asset")
val bevyIconAsset = identityValue("bevy.icon_asset")
val slintIconAsset = identityValue("slint.icon_asset")
val composeLauncherActivity = identityValue("compose.launcher_activity")
val bevyLauncherActivity = identityValue("bevy.launcher_activity")
val slintLauncherActivity = identityValue("slint.launcher_activity")
val productIcons = mapOf(
    "compose" to (composeIconAsset to composeIconResource),
    "bevy" to (bevyIconAsset to bevyIconResource),
    "slint" to (slintIconAsset to slintIconResource),
)

fun launcherActivityClass(activity: String): String =
    if (activity.startsWith('.')) "$androidNamespace$activity" else activity

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = androidNamespace
    // BANDROID-024: latest stable AndroidX/Compose (Compose 1.12.x, core-ktx
    // 1.19.x, lifecycle 2.11.x) require compiling against API 37 or later.
    // compileSdk is independent of targetSdk; runtime behavior stays at 36.
    compileSdk = 37
    ndkVersion = "29.0.14206865"

    defaultConfig {
        applicationId = composeApplicationId
        minSdk = 29
        //noinspection OldTargetApi
        targetSdk = 36
        versionCode = composeVersionCode
        versionName = productVersionName
        // All device tests refuse physical devices and keep the emulator guest
        // offline while the product UI starts (Overview performs an IP check).
        testInstrumentationRunner = "com.musicfrog.infiltrator.OfflineEmulatorTestRunner"
        ndk {
            //noinspection ChromeOsAbiSupport
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    flavorDimensions += "surface"
    productFlavors {
        create("compose") {
            dimension = "surface"
            applicationId = composeApplicationId
            versionCode = composeVersionCode
            versionName = productVersionName
            resValue("string", "app_name", composeDisplayName)
            resValue(
                "string",
                "notification_activity",
                launcherActivityClass(composeLauncherActivity),
            )
        }
        create("bevy") {
            dimension = "surface"
            applicationId = bevyApplicationId
            versionCode = bevyVersionCode
            versionName = productVersionName
            resValue("string", "app_name", bevyDisplayName)
            resValue(
                "string",
                "notification_activity",
                launcherActivityClass(bevyLauncherActivity),
            )
        }
        create("slint") {
            dimension = "surface"
            applicationId = slintApplicationId
            versionCode = slintVersionCode
            versionName = productVersionName
            resValue("string", "app_name", slintDisplayName)
            resValue(
                "string",
                "notification_activity",
                launcherActivityClass(slintLauncherActivity),
            )
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            signingConfig = signingConfigs.getByName("debug")
        }
        debug {
            isMinifyEnabled = false
        }
    }

    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "x86_64")
            isUniversalApk = true
        }
    }

    packaging {
        jniLibs {
            // Extract native libs so they're available in nativeLibraryDir
            // Required for ProcessBuilder to execute mihomo binary
            useLegacyPackaging = true
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
        resValues = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        }
    }

    // BANDROID-021: JVM unit tests (android/app/src/test) run without a device.
    // Android framework calls (e.g. android.util.Log) return defaults so pure
    // state logic does not need Robolectric.
    testOptions {
        unitTests.isReturnDefaultValues = true
        animationsDisabled = true
        managedDevices {
            localDevices {
                create("pixel6Api36") {
                    device = "Pixel 6"
                    apiLevel = 36
                    systemImageSource = "aosp"
                    testedAbi = "x86_64"
                    require64Bit = true
                }
            }
        }
    }
}

androidComponents {
    onVariants { variant ->
        val product = variant.productFlavors
            .singleOrNull { (dimension, _) -> dimension == "surface" }
            ?.second
            ?: return@onVariants
        val (iconAsset, iconResource) = productIcons.getValue(product)
        val generationTask = tasks.register<GenerateProductIcon>(
            "generate${variant.name.replaceFirstChar { it.uppercase() }}ProductIcon",
        ) {
            sourceIcon.set(rootProject.layout.projectDirectory.file("../$iconAsset"))
            resourceName.set(iconResource)
            outputDirectory.set(layout.buildDirectory.dir("generated/productIdentity/${variant.name}/res"))
        }
        variant.sources.res?.addGeneratedSourceDirectory(
            generationTask,
            GenerateProductIcon::outputDirectory,
        )
    }
}

// Restore each Android executable through the packaging lock before copying
// only the verified libmihomo.so into Android's native library source set.
val fetchMihomoScript = rootProject.file("../scripts/fetch-mihomo.py")
val mihomoStagingDirectory = layout.buildDirectory.dir("generated/mihomo-assets")

fun fetchMihomoAsset(taskName: String, target: String, abi: String) =
    tasks.register<Exec>(taskName) {
        commandLine(
            "python3",
            fetchMihomoScript.absolutePath,
            "--target",
            target,
            "--stage",
            mihomoStagingDirectory.get().file("$abi/libmihomo.so").asFile.absolutePath,
        )
    }

val fetchMihomoArm64Asset = fetchMihomoAsset(
    "fetchMihomoArm64Asset",
    "aarch64-linux-android",
    "arm64-v8a",
)
val fetchMihomoX86_64Asset = fetchMihomoAsset(
    "fetchMihomoX86_64Asset",
    "x86_64-linux-android",
    "x86_64",
)

tasks.register<Copy>("prepareMihomoAsset") {
    dependsOn(fetchMihomoArm64Asset, fetchMihomoX86_64Asset)
    from(mihomoStagingDirectory.get().dir("arm64-v8a")) {
        include("libmihomo.so")
        into("arm64-v8a")
    }
    from(mihomoStagingDirectory.get().dir("x86_64")) {
        include("libmihomo.so")
        into("x86_64")
    }
    into(layout.projectDirectory.dir("src/main/jniLibs"))
}

tasks.register<Exec>("cargoBuild") {
    val scriptShPath = rootProject.file("../scripts/android-build.sh").absolutePath

    // 3. 从 local.properties 或环境读取
    val localProperties = Properties()
    val localFile = rootProject.file("local.properties")
    if (localFile.exists()) {
        localFile.inputStream().use { input ->
            localProperties.load(input)
        }
    }
    
    val sdkDir = localProperties.getProperty("sdk.dir")?.takeIf { it.isNotBlank() }
    val ndkDir = localProperties.getProperty("ndk.dir")?.takeIf { it.isNotBlank() }

    // Keep CI-provided SDK/NDK variables when local.properties is absent; an
    // empty Gradle environment value would hide setup-android/setup-ndk.
    if (sdkDir != null) {
        environment("ANDROID_SDK_ROOT", sdkDir)
        environment("ANDROID_HOME", sdkDir)
    }
    if (ndkDir != null) {
        environment("ANDROID_NDK_HOME", ndkDir)
        environment("ANDROID_NDK_ROOT", ndkDir)
    }

    // 4. 构建脚本只保留 sh（Windows 走 Git Bash / WSL 的 bash）；release
    // APK 必须携带 release 优化的 Rust native libraries。
    commandLine("bash", scriptShPath, "--release")

    standardOutput = System.out
    errorOutput = System.err
}

tasks.register<Exec>("verifyAndroidProductIdentity") {
    workingDir = rootProject.projectDir.parentFile
    commandLine(
        "python3",
        rootProject.file("../scripts/quality/android-product-identity-guard.py").absolutePath,
    )
}

tasks.register<Exec>("verifyAndroidMihomoAssets") {
    workingDir = rootProject.projectDir.parentFile
    commandLine(
        "python3",
        rootProject.file("../scripts/quality/android-mihomo-assets-guard.py").absolutePath,
    )
}

tasks.named("preBuild") {
    dependsOn("prepareMihomoAsset")
    dependsOn("cargoBuild")
    dependsOn("verifyAndroidProductIdentity")
    dependsOn("verifyAndroidMihomoAssets")
}

// The Slint NativeActivity is linked into the same Rust library as the
// existing Android bridge. Its flavor-specific library enables `slint-ui`;
// Compose and Bevy continue to package the standard host library.
val buildSlintAndroidRust = tasks.register<Exec>("buildSlintAndroidRust") {
    workingDir = rootProject.projectDir.parentFile
    commandLine("bash", rootProject.file("../scripts/android-slint-build.sh").absolutePath)
    // The shared host task also writes libinfiltrator_android.so. Serialize
    // feature-specific builds so Cargo's per-target output cannot be copied
    // into the wrong Android source set.
    mustRunAfter("cargoBuild")
}
tasks.configureEach {
    if (name.startsWith("mergeSlint") && name.endsWith("JniLibFolders")) {
        dependsOn(buildSlintAndroidRust)
    }
}

// 5. 确保 Kotlin 编译前 Rust 库已准备好 (UniFFI 生成代码需要)
tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
    dependsOn("cargoBuild")
}

dependencies {
    // Only the Bevy product flavor packages the native Activity driver.
    add("bevyImplementation", project(":bevy-host"))

    // BANDROID-024: Android dependency baseline tracks the latest stable
    // releases. Compose artifacts are versioned by the BOM; the rest are pinned
    // explicitly. Re-check Google Maven / Maven Central before changing these.
    implementation(platform("androidx.compose:compose-bom:2026.09.00"))
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.compose.material3:material3-window-size-class")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    implementation("androidx.core:core-ktx:1.19.1")
    implementation("androidx.appcompat:appcompat:1.8.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    // JNA 依赖必须带 @aar 以加载原生库
    implementation("net.java.dev.jna:jna:5.19.1@aar")

    // BANDROID-021: JVM unit tests (no device required).
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
    // BANDROID-022: instrumented tests (emulator/device only).
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.uiautomator:uiautomator:2.4.0")
    androidTestImplementation(platform("androidx.compose:compose-bom:2026.09.00"))
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
