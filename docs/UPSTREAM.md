# 上游依赖升级

> 层级：L2

版本只以对应清单为准：Rust 工具链用 `rust-toolchain.toml`，Rust 依赖用 `Cargo.toml` / `Cargo.lock`，Mihomo 资产用 `packaging/mihomo-assets.json`，Android 工具链用 Gradle 配置。本文不复制版本号或维护依赖清单。

升级前核对上游变更和平台影响；更新锁定来源后，运行受影响的质量门、行为测试与打包验证，并检查许可证、摘要和回滚路径。Core 版本变更还须用锁定的真实二进制执行 [Mihomo smoke](../scripts/core-backend-smoke-mihomo.sh)。

一次聚焦一个依赖族。工具链或 UI 框架的大版本升级独立评审；未取得对应平台和产物证据时，不写成已验证。平台状态以 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md) 为准。
