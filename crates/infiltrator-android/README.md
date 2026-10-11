# infiltrator-android

> 层级：L3

Android Rust host adapter：UniFFI/JNI、产品组合、VPN application 接线、路由计划和与 Kotlin 宿主的 typed 边界。`:vpn` 进程是 Android Core owner；Compose、Bevy 和 Slint UI 通过宿主接口消费同一事实。

本 crate 不实现页面；Bevy driver 位于 `android/bevy-host/driver`，Slint NativeActivity 由 `slint-ui` feature 启用并实现 shared host adapter。授权、前台通知、TUN FD 和撤销由 Android 系统宿主管理，产品约束见 [Android 文档](../../android/docs/README.md)；Slint 表面见 [infiltrator-slint](../infiltrator-slint/README.md)。
