import java.util.Properties

val defaultVersionName = "0.20.0"
val defaultVersionCode = 20_000

val resolvedVersionName = providers.gradleProperty("versionName")
    .orElse(providers.gradleProperty("VERSION_NAME"))
    .orElse(providers.gradleProperty("APP_VERSION"))
    .orElse(providers.environmentVariable("VERSION_NAME"))
    .orElse(providers.environmentVariable("APP_VERSION"))
    .orElse(defaultVersionName)

val resolvedVersionCode = providers.gradleProperty("versionCode")
    .orElse(providers.gradleProperty("VERSION_CODE"))
    .orElse(providers.gradleProperty("APP_VERSION_CODE"))
    .orElse(providers.environmentVariable("VERSION_CODE"))
    .orElse(providers.environmentVariable("APP_VERSION_CODE"))
    .orElse(defaultVersionCode.toString())
    .map { value ->
        value.toIntOrNull()
            ?: error("versionCode must be an integer, got '$value'")
    }

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.musicfrog.infiltrator"
    compileSdk = 36
    ndkVersion = "29.0.14206865"

    defaultConfig {
        applicationId = "com.musicfrog.infiltrator"
        minSdk = 29
        //noinspection OldTargetApi
        targetSdk = 36
        versionCode = resolvedVersionCode.get()
        versionName = resolvedVersionName.get()
        ndk {
            //noinspection ChromeOsAbiSupport
            abiFilters += listOf("arm64-v8a", "x86_64")
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
}

// 1. 定义外部资源路径
val vendorMihomoArm64 = rootProject.file("../vendor/mihomo-android-arm64-v8")
val vendorMihomoAmd64 = rootProject.file("../vendor/mihomo-android-amd64")

val jniDirArm64 = layout.projectDirectory.dir("src/main/jniLibs/arm64-v8a")
val jniDirX86_64 = layout.projectDirectory.dir("src/main/jniLibs/x86_64")

// 2. 修正 Copy 任务：使用标准 Copy 任务类型以兼容配置缓存
tasks.register<Copy>("prepareMihomoAsset") {
    if (vendorMihomoArm64.exists()) {
        from(vendorMihomoArm64) {
            into("arm64-v8a")
            rename { "libmihomo.so" }
        }
    }
    if (vendorMihomoAmd64.exists()) {
        from(vendorMihomoAmd64) {
            into("x86_64")
            rename { "libmihomo.so" }
        }
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

tasks.named("preBuild") {
    dependsOn("prepareMihomoAsset")
    dependsOn("cargoBuild")
}

// 5. 确保 Kotlin 编译前 Rust 库已准备好 (UniFFI 生成代码需要)
tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
    dependsOn("cargoBuild")
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.12.01"))
    implementation("androidx.activity:activity-compose:1.12.2")
    implementation("androidx.compose.material3:material3-window-size-class")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    implementation("androidx.core:core-ktx:1.17.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    // JNA 依赖必须带 @aar 以加载原生库
    implementation("net.java.dev.jna:jna:5.18.1@aar")
}