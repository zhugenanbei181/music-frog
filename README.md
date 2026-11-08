# Infiltrator

> 层级：L1

Rust 跨平台代理客户端，统一管理 Mihomo 与 sing-box 内核。Core 生命周期、配置、版本和控制请求由 Rust 统一管理；UI 提交意图并展示结果。

### 官方产品矩阵

桌面与移动端提供同权、独立发行的各端产品，全线正规化命名：
- **Ice**：Iced 桌面端产品；
- **Lurker**：Bevy UI 跨平台产品（桌面端与 Android 端）；
- **Source**：原生 Android Compose 客户端；
- **Lumen**：Slint 跨平台产品（桌面端与 Android 端）。

各端均复用现有中立 Core 与 Android/Desktop 唯一 owner；每个页面的可操作范围与尚未接入的 owner 能力见 [Slint 表面契约](crates/infiltrator-slint/docs/SURFACE.md)，平台构建和验证状态见 [平台矩阵](docs/PLATFORM_MATRIX.md)。

产品覆盖配置与订阅、代理选择、DNS/TUN、运行态诊断和 WebDAV 同步。基本操作见 [使用说明](crates/infiltrator-contract/docs/README.md)。

## 构建

```bash
bash scripts/fetch-mihomo.sh
cargo build --workspace
```

测试见 [TESTING.md](TESTING.md)，文档入口见 [docs/README.md](docs/README.md)。
