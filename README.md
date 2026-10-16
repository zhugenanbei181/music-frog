# MusicFrog Infiltrator

> 层级：L1

Rust 跨平台代理客户端，统一管理 Mihomo 与 sing-box 内核。Core 生命周期、配置、版本和控制请求由 Rust 统一管理；UI 提交意图并展示结果。

桌面提供同权、独立发行的 Iced、Bevy 与 Slint 产品；Android 提供 Compose、Bevy 与 Slint flavor。Slint 已吸收 Lumen 的完整页面，并复用现有桌面/Android owner；每个页面的可操作范围与尚未接入的 owner 能力见 [Slint 表面契约](crates/infiltrator-slint/docs/SURFACE.md)，平台构建和验证状态见 [平台矩阵](docs/PLATFORM_MATRIX.md)。

产品覆盖配置与订阅、代理选择、DNS/TUN、运行态诊断和 WebDAV 同步。基本操作见 [使用说明](crates/infiltrator-contract/docs/README.md)。

## 构建

```bash
bash scripts/fetch-mihomo.sh
cargo build --workspace
```

测试见 [TESTING.md](TESTING.md)，文档入口见 [docs/README.md](docs/README.md)。
