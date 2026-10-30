import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import java.util.Properties

abstract class GenerateProductIcon : DefaultTask() {
    @get:Nested
    abstract val densities: ListProperty<ProductIconDensity>

    @get:Input
    abstract val resourceName: Property<String>

    @get:OutputDirectory
    abstract val outputDirectory: DirectoryProperty

    @TaskAction
    fun generate() {
        val name = resourceName.get()
        densities.get().forEach { density ->
            val asset = density.asset.get().asFile
            check(asset.isFile) { "Product icon density variant is missing: $asset" }
            val target = outputDirectory.get().file("drawable-${density.density.get()}/$name.png").asFile
            target.parentFile.mkdirs()
            asset.copyTo(target, overwrite = true)
        }
    }
}

abstract class ProductIconDensity {
    @get:Input
    abstract val density: Property<String>

    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val asset: RegularFileProperty
}

val productIdentityFile = rootProject.file("../product-identity.properties")
check(productIdentityFile.isFile) {
    "Android product identity file is missing: $productIdentityFile"
}
val productIdentity =
    Properties().apply {
        productIdentityFile.inputStream().use { load(it) }
    }

fun identityValue(key: String): String = productIdentity.getProperty(key)?.takeIf { it.isNotBlank() }
    ?: error("Missing Android product identity key '$key' in $productIdentityFile")

fun generatedIdentityValue(key: String): String {
    val output =
        providers.exec {
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
    return output.standardOutput.asText
        .get()
        .trim()
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
val productIconDensities = productIdentity.getProperty("product.android_icon_density")
    ?.split(',')
    ?.map { entry -> entry.trim().substringBefore(':') }
    ?.filter { density -> density.isNotEmpty() }
    ?.takeIf { densities -> densities.isNotEmpty() }
    ?: error("Missing or empty product.android_icon_density in $productIdentityFile")
val bevyIconResource = identityValue("bevy.icon_resource")
val slintIconResource = identityValue("slint.icon_resource")
val composeLauncherActivity = identityValue("compose.launcher_activity")
val bevyLauncherActivity = identityValue("bevy.launcher_activity")
val slintLauncherActivity = identityValue("slint.launcher_activity")

val androidToolchainFile = rootProject.file("toolchain.properties")
check(androidToolchainFile.isFile) { "Android toolchain pins are missing: $androidToolchainFile" }
val androidToolchain =
    Properties().apply {
        androidToolchainFile.inputStream().use(::load)
    }

fun toolchainValue(key: String): String = androidToolchain.getProperty(key)?.takeIf { it.isNotBlank() }
    ?: error("Missing Android toolchain key '$key' in $androidToolchainFile")

fun toolchainInt(key: String): Int = toolchainValue(key).toIntOrNull()
    ?: error("Android toolchain key '$key' must be an integer")
val javaTargetVersion = toolchainValue("java.major")

fun launcherActivityClass(activity: String): String = if (activity.startsWith('.')) "$androidNamespace$activity" else activity

// ---------------------------------------------------------------------------
// Release signing
//
// The release identity is the user's persistent keystore and nothing else.
// The keystore itself is materialised by whoever holds the secret (the release
// workflow decodes the base64 blob into a private directory outside the
// repository and hands the path over); Gradle only consumes the path and the
// three credentials. The outcome is a typed sum: either a complete keystore
// or the exact list of what is missing. A release artifact is never signed
// with the debug key and never published unsigned — requesting a release
// variant without the material fails the build.
// ---------------------------------------------------------------------------

/** The release signing outcome: a complete keystore, or what is still missing. */
sealed interface ReleaseSigningOutcome

/** A complete release keystore: the only thing that may sign a release artifact. */
class ReleaseKeystore(val storeFile: File, val storePassword: String, val keyAlias: String, val keyPassword: String) :
    ReleaseSigningOutcome

/** The release signing material is incomplete; the list says exactly what is missing. */
class MissingReleaseMaterial(val missing: List<String>) : ReleaseSigningOutcome

fun releaseSigningValue(property: String, environment: String): String? =
    providers.gradleProperty(property).orNull?.takeIf { it.isNotBlank() }
        ?: providers.environmentVariable(environment).orNull?.takeIf { it.isNotBlank() }

val releaseKeystorePath =
    releaseSigningValue("releaseKeystorePath", "RELEASE_KEYSTORE_PATH")
val releaseStorePassword =
    releaseSigningValue("releaseKeystorePassword", "RELEASE_KEYSTORE_PASSWORD")
val releaseKeyAlias =
    releaseSigningValue("releaseKeyAlias", "RELEASE_KEY_ALIAS")
val releaseKeyPassword =
    releaseSigningValue("releaseKeyPassword", "RELEASE_KEY_PASSWORD")

val releaseSigning: ReleaseSigningOutcome =
    when {
        releaseKeystorePath == null ->
            MissingReleaseMaterial(listOf("RELEASE_KEYSTORE_PATH (the release keystore)"))

        !File(releaseKeystorePath).isFile ->
            MissingReleaseMaterial(listOf("the release keystore file $releaseKeystorePath does not exist"))

        releaseStorePassword == null ->
            MissingReleaseMaterial(listOf("RELEASE_KEYSTORE_PASSWORD"))

        releaseKeyAlias == null ->
            MissingReleaseMaterial(listOf("RELEASE_KEY_ALIAS"))

        releaseKeyPassword == null ->
            MissingReleaseMaterial(listOf("RELEASE_KEY_PASSWORD"))

        else ->
            ReleaseKeystore(
                File(releaseKeystorePath),
                releaseStorePassword,
                releaseKeyAlias,
                releaseKeyPassword,
            )
    }

fun taskMayProduceReleaseArtifact(taskName: String): Boolean {
    val simple = taskName.substringAfterLast(':')
    if (simple.contains("Test", ignoreCase = true)) {
        return false
    }
    if (simple.equals("build", ignoreCase = true)) {
        return true
    }
    val producesArtifact = listOf("assemble", "package", "bundle", "install")
        .any { prefix -> simple.startsWith(prefix, ignoreCase = true) }
    if (!producesArtifact) {
        return false
    }
    val selectsRelease = simple.contains("Release", ignoreCase = true)
    val selectsDebug = simple.contains("Debug", ignoreCase = true)
    return selectsRelease || !selectsDebug
}

// Variant and aggregate Release tasks fail before Gradle can fall back to
// debug signing or emit an unsigned release artifact.
val releaseArtifactRequested =
    gradle.startParameter.taskNames.any(::taskMayProduceReleaseArtifact)

if (releaseArtifactRequested) {
    val missing = (releaseSigning as? MissingReleaseMaterial)?.missing
    if (missing != null) {
        error(
            "Release artifacts require the persistent release keystore. " +
                "Missing: ${missing.joinToString(", ")}",
        )
    }
}

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
    // BANDROID-044.2: ktlint and Detekt are enforced gates for every Kotlin
    // source set of this module. Neither tool uses a baseline, a per-file
    // suppression or a raised issue threshold.
    id("org.jlleitschuh.gradle.ktlint")
    id("io.gitlab.arturbosch.detekt")
}

android {
    namespace = androidNamespace
    // BANDROID-024: compile SDK is independent from the registered target SDK.
    // Shared SDK values live in android/toolchain.properties.
    compileSdk = toolchainInt("android.compile.sdk")
    ndkVersion = toolchainValue("android.ndk.version")

    defaultConfig {
        applicationId = composeApplicationId
        minSdk = toolchainInt("android.min.sdk")
        //noinspection OldTargetApi
        targetSdk = toolchainInt("android.target.sdk")
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

    signingConfigs {
        // The release identity comes from the user-held keystore only. It is
        // created only when the material is complete, so a release build can
        // never sign with the debug key.
        if (releaseSigning is ReleaseKeystore) {
            create("release") {
                storeFile = releaseSigning.storeFile
                storePassword = releaseSigning.storePassword
                keyAlias = releaseSigning.keyAlias
                keyPassword = releaseSigning.keyPassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            if (releaseSigning is ReleaseKeystore) {
                signingConfig = signingConfigs.getByName("release")
            }
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
        sourceCompatibility = JavaVersion.toVersion(javaTargetVersion)
        targetCompatibility = JavaVersion.toVersion(javaTargetVersion)
    }

    kotlin {
        compilerOptions {
            jvmTarget.set(
                org.jetbrains.kotlin.gradle.dsl.JvmTarget
                    .fromTarget(javaTargetVersion),
            )
        }
    }

    // BANDROID-044.1: Android Lint is a real gate for the Compose, Bevy and
    // Slint Debug variants. It never runs `assemble` as a proxy, it never
    // reads a baseline file, and a warning is an error so that a new finding
    // cannot slip through. The narrow, documented suppressions live in
    // `android/app/lint.xml` and are re-checked by
    // `scripts/quality/android-architecture-redline-guard.py`.
    lint {
        abortOnError = true
        warningsAsErrors = true
        checkDependencies = true
        checkReleaseBuilds = false
        checkTestSources = true
        ignoreTestSources = false
        // No baseline: the module must not carry a `lint-baseline.xml`.
        baseline = null
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

// ---------------------------------------------------------------------------
// BANDROID-044.1 / BANDROID-044.2: the static analysis gates.
//
// `lintComposeDebug`, `lintBevyDebug` and `lintSlintDebug` are the AGP Lint
// tasks of the three product flavors; `androidLintGate` is the stable
// aggregate the CI workflow and the local check entry point call, so a
// renamed AGP task cannot silently turn the gate into a no-op.
//
// `ktlintCheck` and `detekt` enforce the Kotlin style and static analysis
// rules. Neither tool is allowed a baseline, a per-file suppression or a
// raised issue threshold.
// ---------------------------------------------------------------------------
val lintGateVariants = listOf("ComposeDebug", "BevyDebug", "SlintDebug")

tasks.register("androidLintGate") {
    group = "verification"
    description = "Runs Android Lint for the Compose, Bevy and Slint Debug variants."
    lintGateVariants.forEach { variant ->
        dependsOn(tasks.named("lint$variant"))
    }
}

ktlint {
    version = "1.8.0"
    // The Android tree is written in the IntelliJ "official" Kotlin style
    // (android/gradle.properties sets `kotlin.code.style=official`), so ktlint
    // verifies that style instead of imposing a second one. The
    // `ktlint_official` style re-indents the whole product surface and its
    // `standard:indent` rule then disagrees with its own formatter.
    additionalEditorconfig.put("ktlint_code_style", "intellij_idea")
    additionalEditorconfig.put("max_line_length", "140")
    // Jetpack Compose composable functions are PascalCase by convention.
    additionalEditorconfig.put(
        "ktlint_function_naming_ignore_when_annotated_with",
        "Composable",
    )
    // The typed owners name their enum entries in UpperCamel, and the binary
    // target observations keep the Rust target triple spelling.
    additionalEditorconfig.put(
        "ktlint_enum_entry_name_casing",
        "upper_or_camel_cases",
    )
    // The generated UniFFI binding is not hand-written source: it is produced
    // by `scripts/android-build.sh` and is excluded from every gate, exactly
    // like `scripts/quality/kt-line-guard.py` excludes it.
    filter {
        exclude { element -> element.file.path.contains("infiltrator_android") }
    }
    // BANDROID-044.2: no baseline, no per-file suppression.
    ignoreFailures = false
}

detekt {
    buildUponDefaultConfig = true
    allRules = false
    config.setFrom(rootProject.file("config/detekt/detekt.yml"))
    // BANDROID-044.2: no baseline, no per-file suppression, no raised
    // threshold. `maxIssues` stays at its default of 0, so a single finding
    // fails the build.
    ignoreFailures = false
    parallel = true
    source.setFrom(
        files(
            "src/main/java",
            "src/test/java",
            "src/androidTest/java",
            "src/androidTestCompose/java",
        ).asFileTree.matching {
            include("**/*.kt")
            exclude("**/infiltrator_android/**")
        },
    )
}

androidComponents {
    onVariants { variant ->
        val product =
            variant.productFlavors
                .singleOrNull { (dimension, _) -> dimension == "surface" }
                ?.second
                ?: return@onVariants
        val iconResource = identityValue("$product.icon_resource")
        val generationTask =
            tasks.register<GenerateProductIcon>(
                "generate${variant.name.replaceFirstChar { it.uppercase() }}ProductIcon",
            ) {
                densities.set(
                    productIconDensities.map { density ->
                        val variantIcon = project.objects.newInstance<ProductIconDensity>()
                        variantIcon.density.set(density)
                        variantIcon.asset.set(
                            rootProject.layout.projectDirectory.file(
                                "../crates/infiltrator-shared/assets/product/icon-android-$density.png",
                            ),
                        )
                        variantIcon
                    },
                )
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

fun fetchMihomoAsset(taskName: String, target: String, abi: String) = tasks.register<Exec>(taskName) {
    val stagingFile = mihomoStagingDirectory.get().file("$abi/libmihomo.so")
    commandLine(
        "python3",
        fetchMihomoScript.absolutePath,
        "--target",
        target,
        "--stage",
        stagingFile.asFile.absolutePath,
    )
}

val fetchMihomoArm64Asset =
    fetchMihomoAsset(
        "fetchMihomoArm64Asset",
        "aarch64-linux-android",
        "arm64-v8a",
    )
val fetchMihomoX86_64Asset =
    fetchMihomoAsset(
        "fetchMihomoX86_64Asset",
        "x86_64-linux-android",
        "x86_64",
    )

tasks.register<Copy>("prepareMihomoAsset") {
    mustRunAfter("cargoBuild")
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
val buildSlintAndroidRust =
    tasks.register<Exec>("buildSlintAndroidRust") {
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
