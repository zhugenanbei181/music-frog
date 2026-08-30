# infiltrator-android

## 1. Role (单一职责与定位)
Android 平台端 UniFFI 绑定层与原生生命周期桥接，负责向 Android Kotlin (Jetpack Compose) 提供强类型不可变 Snapshot，管理 VpnService 路由与原生 JNI 通信。

## 2. Boundary (依赖边界与禁止耦合)
- 依赖上游: `infiltrator-core`, `mihomo-platform`, `infiltrator-shared`
- 禁止反向依赖: 禁止反向依赖桌面端 (`infiltrator-desktop`, `infiltrator-iced`, `src-tauri`)
- 零跨 crate re-export 原则: 必须通过规范路径导入，禁止任意 glob 转发。

## 3. Contract (核心公开类型与对外契约)
- `AndroidHost` & `AndroidApi`: UniFFI 对外导出的核心桥接接口。
- `AppDomainSnapshot`: 强类型不可变领域状态快照。
- `ChinaIpBypass` & `VpnRoutePlan`: Android VPN 路由表生成与 Private DNS 治理。

## 4. Verification (验证与测试指引)
- 运行测试（工作区全量统一入口）: `bash scripts/test.sh`
- 质量门禁: 遵守 `line-guard.py`（单文件 ≤ 800 行）与 `doc-link-guard.py`
