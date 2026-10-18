# Android 工程地图

> 层级：L3

`:app` 提供 Compose 与 Bevy 两种 flavor；共享 Kotlin host 和 VPN/control service 位于 `app/src/main`。Bevy launcher 位于 `app/src/bevy`，NativeActivity Gradle module 位于 `bevy-host`。

Rust Android owner 与 UniFFI 接口见 [infiltrator-android](../crates/infiltrator-android/README.md)；Bevy driver 见 [driver README](bevy-host/driver/README.md)。本地产品章程见 [docs/README.md](docs/README.md)。

Android 构建共享版本登记于 [toolchain.properties](toolchain.properties)：Gradle app/Bevy host/JVM harness、tag-only CI、模拟器脚本和 Bevy 诊断构建从同一组 SDK/NDK/JDK pins 读取。Gradle wrapper 与 Rust 版本分别以 `gradle/wrapper/gradle-wrapper.properties` 和根 `rust-toolchain.toml` 为唯一来源。本机运行 Android 构建前执行 `python3 scripts/android-toolchain.py check`；它会核对 JDK、Gradle wrapper、SDK/command-line tools、NDK revision 与 Rust toolchain。

当前兼容组合为 AGP 9.3.1、Kotlin Gradle Plugin 2.4.21、Gradle 9.7.0 和 JDK 17。Kotlin 官方 fully-supported 矩阵将 KGP 2.4.21 的上限列为 Gradle 9.7.0 / AGP 9.3.1；AGP 9.3 至少要求 Gradle 9.5，并支持 API 37，因此现有 compile SDK 37 与该组合匹配。[Kotlin Gradle compatibility](https://kotlinlang.org/docs/gradle-configure-project.html)、[AGP 9.3 release notes](https://developer.android.com/build/releases/agp-9-3-0-release-notes)、[Gradle 9.7.0 release notes](https://docs.gradle.org/9.7.0/release-notes.html)。AGP 9.3.2 才修复 9.3 系列的 JDK 17 Lint crash；KGP 官方兼容表目前只列到 AGP 9.3.1，所以 Android Lint 暂不启用，待兼容矩阵更新或有独立行为证据后再推进。

## :app flavors

`:app` packages the Compose and Bevy product flavors. `src/main` owns shared Android services, permissions and manifest declarations; `src/compose` and `src/bevy` own flavor-specific launchers and resources. The Bevy NativeActivity driver comes from `:bevy-host`. Product identity comes from the root workspace version and `product-identity.properties`. Keep Kotlin as the Android lifecycle/IPC/UI adapter; Rust ownership and cross-process rules are in [BEVY_ANDROID_PRODUCT.md](docs/BEVY_ANDROID_PRODUCT.md).

## bevy-host module

Gradle module 为 Bevy flavor 打包 NativeActivity 与 Rust driver；Compose flavor 不带该 driver。两种 flavor 共用 `:app` 的 VPN/control host，本目录的构建由 `android/app` Gradle 变体驱动。driver entry 行为与 lockfile 约定见 [driver README](bevy-host/driver/README.md)；依赖闭包边界与迁移状态见 [Android 产品章程](docs/BEVY_ANDROID_PRODUCT.md)。
