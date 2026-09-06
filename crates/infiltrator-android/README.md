# infiltrator-android

## 1. Role (单一职责与定位)
Android 平台端 UniFFI 绑定层与原生生命周期桥接，负责向 Android Kotlin (Jetpack Compose) 提供强类型不可变 Snapshot，管理 VpnService 路由与原生 JNI 通信。

## 2. Boundary (依赖边界与禁止耦合)
- 依赖上游: `infiltrator-core`, `mihomo-platform`, `infiltrator-shared`
- 禁止反向依赖: 禁止反向依赖桌面端 (`infiltrator-desktop`, `infiltrator-iced`, `src-tauri`)
- 零转发导入原则: 禁止一切 re-export（`pub use` / `pub(crate) use` 转发层，含 glob），禁止 `use ... as 别名`（`as _` 匿名 trait 导入除外）；一切导入走定义模块的规范路径。由 `scripts/quality/import-guard.py` 在 CI 强制。

## 3. Contract (核心公开类型与对外契约)
- `AndroidHost` & `AndroidApi`: UniFFI 对外导出的核心桥接接口。
- `AppDomainSnapshot`: 强类型不可变领域状态快照。
- `ChinaIpBypass` & `VpnRoutePlan`: Android VPN 路由表生成与 Private DNS 治理。
- `VpnSessionSnapshot` / `VpnServicePort` / `VpnServiceApplication`: 统一权限、前台服务、隧道 FD、tun2proxy 与撤销状态；Kotlin/Compose 不维护第二套 VPN 生命周期。

## 4. VpnService lifecycle contract

Android 原生宿主必须按系统生命周期把事实传入这条 Rust seam：

1. 用户点击启动后调用 `VpnService.prepare(Context)`；返回非空 Intent 时先完成用户授权。
2. 授权成功后启动 `VpnService`，立即提升前台通知；service 在 `Builder.establish()` 前调用 UniFFI `prepare_vpn()`，由 Rust application 通过 bridge 下发统一 JSON 配置，宿主据此设置地址、路由、DNS、MTU。
3. 将建立后的 TUN file descriptor 传给 UniFFI `start_vpn(fd)`；此调用只启动 tun2proxy，Rust 只有在前台 readback 成功后才报告 `Running`。
4. `onRevoke()` 必须调用 `revoke_vpn()`，用户停止或 service 销毁调用 `stop_vpn()`；`vpn_session_status()` 用于回读 `Running/Stopped/Revoked/Failed`。

目标 SDK 34+ 的 manifest 还必须声明 `BIND_VPN_SERVICE` 的 VPN service、匹配 `android.net.VpnService` intent，并声明适用的 foreground-service 权限和类型；具体通知与 Context 代码属于 Android host，不进入 Rust application/domain。

## 5. Verification (验证与测试指引)
- 运行测试（工作区全量统一入口）: `bash scripts/test.sh`
- 质量门禁: 遵守 `line-guard.py`（单文件 ≤ 800 行）与 `doc-link-guard.py`
