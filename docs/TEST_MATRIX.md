# 跨平台与多 UI 回归矩阵

`TESTING.md` 负责“怎么运行”；本文件负责“每个功能必须在哪一层证明”。编译通过、mock 测试通过、真实 mihomo smoke 和目标设备验证是四种不同证据。

## 1. 证据层级

| 层级 | 允许的副作用 | 证明什么 | 不能证明什么 |
| --- | --- | --- | --- |
| L0 纯逻辑 | 无进程、无网络、无 UI | 解析、校验、状态机、错误映射、版本比较 | controller 真正可用 |
| L1 mock controller | loopback mock HTTP/WebSocket | Rust API/use-case、重试、取消、generation、协议错误 | mihomo 真实版本行为 |
| L2 surface behavior | UI headless 或浏览器组件测试 | intent 到 UI 的可达性、状态投影、文案和失败分支 | OS 权限、托盘、VPN、GPU |
| L3 package/core smoke | 明确的二进制、资源和 loopback/隔离网络 | 资产、启动参数、readiness、真实 API/config 兼容 | 所有真实设备和桌面环境 |
| L4 platform evidence | 目标设备/桌面系统 | VPN、托盘、系统代理、后台、窗口、ABI 和安装行为 | 其他平台的完成状态 |

普通 Rust/Web/Android 单元测试默认只允许 L0-L2。L3/L4 必须是显式 stage，并在报告中写明 core 版本、平台、架构和失败原因。

### L2 像素证据（iced 桌面，niri 后台捕获）

`scripts/capture-iced.sh` 在后台合成器栈（`kwin_wayland --virtual` 不可见宿主 + 嵌套 niri，软件渲染）中以 demo fixture（mock mihomo 数据源，无生产副作用）渲染真实二进制，按 页面×皮肤 捕获窗口截图（9 页 × 亮/暗 = 18 场景），产出 `docs/screenshots/iced/` 与 manifest（尺寸/字节/sha256 收据）。这是 L2「状态投影、文案和失败分支」的像素级证据；CI 不运行捕获（本地证据），失败语义区分 `BLOCKED (compositor)` 与场景 `FAIL`。

## 2. 功能域矩阵

| 功能域 | L0 | L1 mock API | L2 UI | L3 真实 core | L4 平台 |
| --- | --- | --- | --- | --- | --- |
| Core 启停/readiness | 状态机、错误 | start/stop、超时、旧 generation | 三端 loading/failed/retry | pinned mihomo 启动、`/version`、配置读取 | 进程、端口、后台退出 |
| Profile/订阅 | 名称、URL、YAML 校验 | save/switch/update/rebuild | Iced/Web/Android 表面一致 | profile 应用后 core reload | 文件路径、权限、并发实例 |
| Proxy/路由/delay | mode/group/node/result | proxy API、批量 delay、取消 | 列表、排序、不可用提示 | real proxy group/provider | 网络环境、超时、VPN |
| Runtime diagnostics | DTO、聚合、bounded queue | connections/logs/traffic/memory/IP | stream 连接、刷新、断线 | controller 实际 stream | 长时间运行、资源上限 |
| DNS/Fake-IP/TUN | typed config、字段校验 | patch/reload/flush | form/raw JSON、权限提示 | 配置字段和实际生效 | TUN/VPN、系统路由、恢复 |
| Rules/providers/sniffer | 解析、排序、JSON 校验 | provider update、reload | 编辑器、dirty/conflict | core schema/运行结果 | 大配置、磁盘和性能 |
| WebDAV sync | 索引/冲突/重试 | loopback DAV mock | 保存/测试/取消/结果 | 不要求真实 core | Android/桌面文件与后台 |
| Core 版本交付 | digest/路径/回滚状态 | 下载器、失败注入 | 进度、版本选择、失败提示 | 官方 pinned asset + health check | MSI/APK/系统包、升级回滚 |

## 3. UI parity 矩阵

每个 intent 至少维护四列：

```text
intent → Rust command/result → Iced decision → Android decision（0.30 起加 Bevy UI decision）
```

三端必须一致的字段：命令含义、目标身份、成功/失败/取消/超时/不支持、revision/generation、用户可见反馈。可以不同的字段：布局、手势、导航、窗口、动画、密度和平台权限流程。

缺少某端能力时，使用 `unsupported(reason)`；有替代路径时使用 `accepted difference(reason)`。禁止只在某端隐藏按钮而不登记决策。

## 4. 执行入口

- Rust 基础测试：`bash scripts/test.sh`；
- Android 编译/UniFFI：`bash scripts/android-build.sh`，再执行 Gradle 对应 variant；
- 真实 core smoke：单独的 release/compatibility stage，不能混入普通单元测试；
- 平台/打包：按 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md) 逐平台记录，不用另一平台的结果代替。

## 5. 通过条件

一个 TODO 只有在其声明的最低层级全部通过后才能标记 `DONE`。如果 L0-L2 已通过但 L3/L4 缺失，状态应为“代码和 mock 已完成，平台/core 待验证”，而不是完成。
