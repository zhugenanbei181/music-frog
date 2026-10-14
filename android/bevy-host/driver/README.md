# infiltrator-bevy-android-driver

> 层级：L3

Android NativeActivity Rust driver，初始化 Bevy UI surface 并提供 Android ABI 动态库。由 `android/bevy-host` Gradle module 构建，不属于根 Cargo workspace。

该独立 Cargo workspace 使用自己的 `Cargo.lock`；Gradle 构建带 `--locked`，依赖变化必须通过锁文件更新审阅。

driver entry 只初始化 Bevy surface，通过 typed IPC 访问 `:vpn` owner，不创建 `CoreApplication`。Bevy 产品 UI 只消费注入的 `SurfaceSnapshot` 与 typed command sink；Android driver 以 `default-features = false` 依赖 UI，不把 `infiltrator-application`、composition 或 capture host 链进产品 `.so`。未接通时显示明确的 `NotReady`。依赖闭包由 [Android driver dependency guard](../../../scripts/quality/android-bevy-driver-dependency-guard.py) 检查。
