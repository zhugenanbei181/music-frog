# Android 文档

> 层级：L3

本目录只记录 Android 产品与宿主规则：[BEVY_ANDROID_PRODUCT.md](BEVY_ANDROID_PRODUCT.md)。

## 消费端与宿主架构纪律

Android 表面与 Native 宿主严格遵照自底向上裁决：所有状态与控制基于底层 Owner 权威读回与 typed 契约；严禁在 NativeActivity、Driver 或 Android IPC 适配层中通过容错分支、默认值或临时标志位修补底层未决状态。

Gradle flavors 和 Rust/NativeActivity 子包见 [Android 工程地图](../README.md)、[infiltrator-android](../../crates/infiltrator-android/README.md)、[Bevy driver](../bevy-host/driver/README.md) 与 [Slint 表面](../../crates/infiltrator-slint/docs/SURFACE.md)。
