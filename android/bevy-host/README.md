# Android Bevy Host

> 层级：L3

Gradle module 为 Bevy flavor 打包 NativeActivity 与 Rust driver；Compose flavor 不带该 driver。两种 flavor 共用 `:app` 的 VPN/control host，本目录的构建由 `android/app` Gradle 变体驱动。

driver entry 行为与 lockfile 约定见 [driver README](driver/README.md)；依赖闭包边界与迁移状态见 [Android 产品章程](../docs/BEVY_ANDROID_PRODUCT.md)。
