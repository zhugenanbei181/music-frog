plugins {
    id("com.android.application") version "9.3.1" apply false
    id("org.jetbrains.kotlin.jvm") version "2.4.21" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21" apply false
    // BANDROID-044.2: ktlint and Detekt are wired as real gates. They are
    // declared here (apply false) so every module resolves the same pinned
    // version; the modules that own Kotlin sources apply them.
    id("org.jlleitschuh.gradle.ktlint") version "14.2.0" apply false
    id("io.gitlab.arturbosch.detekt") version "1.23.8" apply false
}
