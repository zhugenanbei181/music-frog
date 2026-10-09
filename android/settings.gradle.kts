pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "musicfrog-infiltrator-android"
include(":app")
// BANDROID-003/004: the Bevy Android product host module
// (`android/bevy-host`) is intentionally NOT included yet. It requires a
// verified JDK + Gradle + NDK + Rust Android toolchain and would otherwise risk
// the `:app` build. Enable it only after the wiring checklist in
// android/bevy-host/README.md passes:
// include(":bevy-host")
