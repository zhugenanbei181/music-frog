# infiltrator-bevy-android-driver

> 层级：L3

Android NativeActivity Rust driver，初始化 Bevy UI surface 并提供 Android ABI 动态库。由 `android/bevy-host` Gradle module 构建，不属于根 Cargo workspace。

该独立 Cargo workspace 使用自己的 `Cargo.lock`；Gradle 构建带 `--locked`，依赖变化必须通过锁文件更新审阅。

driver entry 只初始化 Bevy surface，通过 typed IPC 访问 `:vpn` owner，不创建 `CoreApplication`，未接通时显示明确的 `NotReady`。依赖闭包边界、presentation helpers 与 desktop launcher 的迁出状态见 [Android 产品章程](../../docs/BEVY_ANDROID_PRODUCT.md)。
