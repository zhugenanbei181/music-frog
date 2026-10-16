# Contract 文档

> 层级：L2

本目录记录由 `infiltrator-contract` 拥有的跨 UI 语义与布局契约。

- [双端语义与场景验收](UI_PARITY_AUDIT.md)：共享场景身份、同异律和证据要求。
- [使用说明](USAGE_SPEC.md)：两端共用的基本操作。
- ProfileDocument codec 的 Mixin Studio 跨端观察由 [`profile_mixin_studio`](../src/profile_mixin_studio.rs) 定义；展示文案由 presentation 投影。
- Profile 展示名由 [`profile_display_name`](../src/profile_display_name.rs) 用 typed Identity/Custom 读写结果表达，稳定 ProfileId 与文件名不变。
- Profile catalog 顺序由 [`profile_catalog_order`](../src/profile_catalog_order.rs) 以完整引擎身份快照、精确目标排列和 Applied/Unchanged/Stale 回执表达。
- 清除活动 profile 选择由 [`profile_selection_clear`](../src/profile_selection_clear.rs) 表达精确期望状态、owner 授权与 `Unselected` 读回；不会暗中停止独立 Core/VPN service。
- Age 私钥由 [`profile_age_credential`](../src/profile_age_credential.rs) 表达不序列化的敏感输入、revisioned 配置状态和显式 owner 授权；owner 执行 Read 时自行读取并原子栅栏当前 active-reference，将确切 selection 与非敏感凭据状态/revision 一并回传，UI 无须从派生 `is_active` 拼出授权上下文；engine/ProfileId 凭据独立于文档是否已创建，支持首次加密订阅导入预置密钥；常规 profile 快照只读取配置状态，不包含密钥。
- 同一契约以非格式化、zeroizing 的 armored/decrypted 内容类型表达 Core 内订阅解密的瞬时载荷；解密结果不是 UI wire/read model。
- 包职责见 [crate README](../README.md)。

## 契约权威性与自底向上裁决铁律

本包所定义的跨 UI 语义契约是所有 Core 与所有 UI 表面（Iced、Bevy、Slint、Android）之间的法定唯一通信边界与真源。

- **先底层契约后上层接入**：遇到新业务场景、底层架构未决或语义不清晰时，必须首先在本契约与 domain/ports 层完成明确裁决，定义不可变的 typed 意图、显式状态机与权威读回；
- **严禁上层私设与修补**：UI 表面严禁自行定义业务结构、严禁以布尔或派生字段推导业务事实、严禁在契约未裁决前在上层进行修补性开发；
- **全端同一律**：所有 UI 表面均平等且严格消费本契约，不得为单一 UI 开辟私有后门或兼容分支。

