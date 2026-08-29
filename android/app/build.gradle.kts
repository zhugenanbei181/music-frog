import java.util.Properties
import java.io.FileInputStream

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.musicfrog.despicableinfiltrator"
    compileSdk = 36
    ndkVersion = "29.0.14206865"

    defaultConfig {
        applicationId = "com.musicfrog.despicableinfiltrator"
        minSdk = 29
        //noinspection OldTargetApi
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            //noinspection ChromeOsAbiSupport
            abiFilters += listOf("arm64-v8a", "x86_64")
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
    
    val sdkDir = localProperties.getProperty("sdk.dir") ?: ""
    val ndkDir = localProperties.getProperty("ndk.dir") ?: ""

    environment("ANDROID_SDK_ROOT", sdkDir)
    environment("ANDROID_HOME", sdkDir)
    environment("ANDROID_NDK_HOME", ndkDir)
    environment("ANDROID_NDK_ROOT", ndkDir)

    // 4. 构建脚本只保留 sh（Windows 走 Git Bash / WSL 的 bash）
    commandLine("bash", scriptShPath)

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