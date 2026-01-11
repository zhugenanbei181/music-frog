plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.musicfrog.despicableinfiltrator"
    compileSdk = 36 // 注意：确保你已安装 Android 16 Preview SDK
    ndkVersion = "29.0.14206865"

    defaultConfig {
        applicationId = "com.musicfrog.despicableinfiltrator"
        minSdk = 26
        //noinspection OldTargetApi
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            //noinspection ChromeOsAbiSupport
            abiFilters += listOf("arm64-v8a", "x86_64")
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

// 2. 修正 Copy 任务：多架构支持
tasks.register("prepareMihomoAsset") {
    // 使用 doLast 确保在执行阶段运行
    doLast {
        if (vendorMihomoArm64.exists()) {
            copy {
                from(vendorMihomoArm64)
                into(jniDirArm64)
                rename { "libmihomo.so" }
            }
        } else {
            logger.warn("Warning: ARM64 binary not found at $vendorMihomoArm64")
        }

        if (vendorMihomoAmd64.exists()) {
            copy {
                from(vendorMihomoAmd64)
                into(jniDirX86_64)
                rename { "libmihomo.so" }
            }
        } else {
            logger.warn("Warning: AMD64 binary not found at $vendorMihomoAmd64")
        }
    }
}

tasks.register<Exec>("cargoBuild") {
    val scriptPath = rootProject.file("../scripts/android-build.ps1").absolutePath
    val scriptShPath = rootProject.file("../scripts/android-build.sh").absolutePath

    // 3. 关键修复：直接从 Android 插件获取最准确的 SDK/NDK 路径
    // 这样无论 local.properties 里写没写，只要 AS 能识别 NDK，这里就能拿到
    val androidExt = project.extensions.getByType(com.android.build.gradle.BaseExtension::class.java)
    val sdkDir = androidExt.sdkDirectory
    val ndkDir = androidExt.ndkDirectory

    environment("ANDROID_SDK_ROOT", sdkDir)
    environment("ANDROID_HOME", sdkDir)
    environment("ANDROID_NDK_HOME", ndkDir)
    environment("ANDROID_NDK_ROOT", ndkDir)

    // 4. 关键修复：不依赖 internal API，使用标准 Java 检测 OS
    val isWindows = System.getProperty("os.name").lowercase().contains("win")

    if (isWindows) {
        commandLine("powershell", "-ExecutionPolicy", "Bypass", "-File", scriptPath)
    } else {
        commandLine("bash", scriptShPath)
    }

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
    // JNA 依赖必须带 @aar 以加载原生库
    implementation("net.java.dev.jna:jna:5.18.1@aar")
}