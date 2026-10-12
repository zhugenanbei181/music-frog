# infiltrator-android

> 层级：L3

Android Rust host adapter：UniFFI/JNI、产品组合、VPN application 接线、路由计划和与 Kotlin 宿主的 typed 边界。`:vpn` 进程是 Android Core owner；Compose、Bevy 和 Slint UI 通过宿主接口消费同一事实。

`AndroidOwnerOperation::CoreVersionManagement` 将 engine-scoped 版本查询、安装、激活、卸载、回滚及安装任务观察/取消送入 `:vpn` 唯一的 `CoreVersionManagementApplication`，并通过 typed enum/record 返回结果。Rust nextest 覆盖该请求、读回和 revision 语义；Kotlin 绑定、APK 与 Android 设备行为仍须单独验收。

Bevy Android owner client 的核心 transport 回归可在不编译 Bevy UI 的情况下运行：`cargo nextest run --offline --locked -p infiltrator-android --test bevy_android_owner_client_core`。测试直接包含 driver 的 owner client 源码，并用测试专用的窄 UI sink/source seam；它验证 typed owner snapshot、scope/readback、终态与超时围栏，不替代真实 driver/UI 集成或设备 IPC 验收。

本 crate 不实现页面；Bevy driver 位于 `android/bevy-host/driver`，Slint NativeActivity 由 `slint-ui` feature 启用并实现 shared host adapter。授权、前台通知、TUN FD 和撤销由 Android 系统宿主管理，产品约束见 [Android 文档](../../android/docs/README.md)；Slint 表面见 [infiltrator-slint](../infiltrator-slint/README.md)。
