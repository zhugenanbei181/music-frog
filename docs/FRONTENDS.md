# 多 UI 求同存异矩阵

本项目的 UI 不是三份产品逻辑。Iced 是主桌面 surface，Tauri + Vue 是兼容/次级管理 surface，Android Compose 是移动伴侣；三者共享用户意图和 Rust 结果，不共享 toolkit 的状态和布局实现。

## 1. 决策标记

| 标记 | 含义 |
| --- | --- |
| `shared` | 命令、数据、错误和生命周期语义共用；实现可在各端独立 |
| `local` | 只属于该端的窗口、手势、布局、导航或宿主能力 |
| `accepted difference` | 有意不同，但对应同一个 shared intent，且有替代路径/原因 |
| `unsupported` | 当前端没有能力或产品价值不足，必须显示 typed 不支持，不得静默隐藏 |

## 2. 功能矩阵

| 用户意图 | Iced 主桌面 | Tauri + Vue | Android Compose | 共享规则 |
| --- | --- | --- | --- | --- |
| 启动/停止/重启 mihomo | `shared` + desktop tray `local` | `shared` + Tauri tray `local` | `shared` + VPN/background `local` | 同一 lifecycle state、failure、generation |
| profile 导入/编辑/删除/切换 | `shared` + 桌面编辑器 `local` | `shared` + Web/外部编辑器 `local` | `shared` + 移动表单 `local` | profile identity、revision 和重建结果一致 |
| 订阅更新 | `shared` | `shared` | `shared` | scheduler、重试、部分失败和通知语义一致 |
| 代理模式/代理组/节点 | `shared` | `shared` | `shared` | mode、group、node、delay 的类型和错误一致 |
| connections/logs/traffic/memory/IP | `shared` | `shared` | `shared` | snapshot/stream 生命周期与不可用状态一致 |
| DNS/Fake-IP/TUN | `shared` + 多栏表单 `local` | `shared` + JSON 深编辑 `local` | `shared` + VPN 权限 `local` | 字段能力来自 Rust capability，不以 UI 默认值补齐 |
| rules/providers/sniffer | `shared` + 原始编辑器 `local` | `shared` + 浏览器编辑器 `local` | `shared` + 移动子页面 `local` | 结构化字段和 raw JSON 的校验规则一致 |
| WebDAV sync | `shared` | `shared` | `shared` | 连接、冲突、取消和结果模型一致 |
| core 下载/安装/切换 | `shared` | `shared` | `accepted difference`：随 APK/ABI 交付 | Android 不提供桌面式任意版本安装时要显式说明 |
| 系统代理/自启动/托盘 | `local` | `local` | `unsupported` 或 Android 系统设置路径 | 不把 OS 动词泄漏到共享领域模型 |
| 视觉动效、布局密度、导航手势 | `local` | `local` | `local` | 允许差异，不得改变功能可达性和失败语义 |

## 3. 共享层与本地层的切分

### 必须进入 Rust shared contract

- 产品意图和命令名；
- 输入校验、目标身份、当前 revision/generation；
- owned response、capability、availability 和错误枚举；
- 异步任务的开始、进度、终止、取消和重试语义；
- profile/config/runtime 的 canonical owner；
- 跨端行为测试需要验证的结果矩阵。

### 必须留在 frontend/host local

- Iced widget、Vue component、Compose composable 和各自的 view model；
- 窗口、托盘、浏览器打开、Android permission/VPN service、输入法和返回栈；
- 适合屏幕尺寸的布局、动画、滚动、手势和密度；
- toolkit 自己的缓存，但缓存只能由 shared revision/generation 失效。

## 4. 当前迁移顺序

1. 以 `mihomo-api::MihomoApi` 和 `CoreController` 为底层 seam，增加面向用户意图的 Rust application facade。
2. 先迁移一条完整链路：profile 切换 → 配置应用 → core readiness → runtime status。
3. Iced 作为主桌面参考实现，Tauri/Web 和 Android 按同一矩阵接入，不复制 Iced 的 `AppState`。
4. 每迁移一个 intent，补三端 decision、mock 行为测试和适用平台验证；缺席端登记 `unsupported`。
5. shared facade 稳定后，逐步删除 UI 对 `mihomo-config`、`mihomo-platform` 和底层 HTTP client 的直接依赖。

## 5. 反分叉检查

- 新按钮是否对应已有 intent？没有就先进入 shared contract。
- 新错误是否来自 Rust typed result？不能在 UI 里靠字符串猜测。
- 同一数据是否在 Iced state、Vue composable、Android ViewModel 各存一份？若是，区分 canonical 与 render cache。
- 某端不支持时是否明确展示原因？不能用空列表、默认值或隐藏导航伪装完成。
- 差异是否有 layout/permission/toolkit 方面的真实理由？没有理由的差异应合并。
