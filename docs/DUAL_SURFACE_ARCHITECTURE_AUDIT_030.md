# 0.30 双 UI 底层架构审计

审计日期：2026-09-05  
审计分支：`main`（本地 `e78b36a`）  
审计范围：`infiltrator-application`、`infiltrator-contract`、`infiltrator-ports`、`infiltrator-composition`、`infiltrator-desktop`、`infiltrator-iced`、`infiltrator-bevy-ui`、`infiltrator-bevy-widgets`

## 结论

结论必须分成两层说：

1. **核心分层原则已经清楚**：领域、契约、端口、应用、组合根和宿主适配器的方向已经成立；`infiltrator-application` 的生产代码没有直接依赖 Tokio，Bevy UI 也没有直接依赖 Tokio、Reqwest 或 Mihomo client。
2. **两个 UI 还没有达到“实现完成且进度对等”**：Iced 仍是“UI + 桌面产品组合”的合体，Bevy 目前只有 Overview 接入真实 application 数据泵，其他业务页仍以 demo projection 挂载。因此现在可以确认的是**目标架构**，不能确认的是**双端功能落地完成**。

这不是否定现有工作，而是把“架构已成立”和“产品已完成”从同一个状态中拆开。后续所有 0.30 任务都以本文件和 [双端主控计划](DUAL_SURFACE_PARITY_MASTER_PLAN.md) 为准。

## 已确认的底层边界

```text
┌──────────────────────────────────────────────────────────────┐
│ Iced UI adapter                 Bevy UI adapter               │
│ View/Update + Iced Task         Scenes/Systems + Bevy ECS     │
│ 只处理本地 view state            只处理 render projection       │
└───────────────┬──────────────────────────┬───────────────────┘
                │ CommandIntent / Snapshot / Event / Capability
                ▼                          ▼
       ┌──────────────────────────────────────────────┐
       │ infiltrator-application                       │
       │ use-case、事务、生命周期、单一 Core 状态源      │
       │ 只依赖 infiltrator-ports                       │
       └──────────────────────┬───────────────────────┘
                              │ ports
             ┌────────────────┴────────────────┐
             ▼                                 ▼
   infiltrator-domain                 infiltrator-contract
   纯模型、算法、校验                  跨端命令、结果、事件、快照
   无 UI / OS / Tokio                 稳定、可序列化、无 toolkit
                              │
                              ▼
       composition roots + outbound/host adapters
       mihomo-api / mihomo-config / mihomo-version
       infiltrator-http / core / desktop / android / ios
```

必须坚持以下解释：

- `Tokio` 可以存在于 composition、HTTP/outbound adapter 和 host adapter；它不能成为 domain、contract、ports、application 的事实模型，也不能从跨端公开 API 泄漏出去。
- Iced 或 Bevy 可以拥有各自 toolkit 的渲染调度和本地 view state，但不能各自拥有一套 Core 事实、订阅更新器、配置写入器或 Mihomo client。
- `infiltrator-desktop`、`infiltrator-android`、`infiltrator-ios` 是与 UI 正交的同级 host adapter。Bevy Android 只是 UI surface，Android VPN 生命周期仍属于 Android host。
- “双端一视同仁”指 shared intent、结果、错误、能力、revision/generation、可达功能一致；窗口、托盘、手势、权限和布局可以是 `local` 或 `accepted difference`。

## 当前代码证据

### 通过项

- `infiltrator-application` 生产依赖树不含 Tokio；Tokio runtime 由 `infiltrator-composition` 注入。
- `infiltrator-domain`、`infiltrator-contract`、`infiltrator-ports` 的具体 UI/传输依赖守卫通过。
- `infiltrator-bevy-ui` 的生产依赖和源码不直接构造 `MihomoClient`、Reqwest 或 Tokio；`controller` 通过 application-owned Overview pump 接入。
- Bevy 的命令入口已经存在：页面提交 `UiCommand`，生产 sink 转为 `CommandIntent`，再交给 `CoreApplication`。

### 尚未通过项

1. **Bevy 业务页还不是 live surface**。`route.rs` 在 `Proxies`、`Profiles`、`Rules`、`Connections`、`Logs`、`Dns`、`Doctor`、`AppRouting`、`Sync`、`Settings` 路由直接使用各自的 `Projection::demo()`；这些页的命令按钮有入口，但页面投影尚未由真实 application snapshot/event 驱动。
2. **Iced 仍包含桌面组合职责**。生产 crate 直接持有 `tokio`、`infiltrator-desktop`、`infiltrator-admin`，启动入口还负责单实例、崩溃清理、文件系统、托盘和 Admin server glue。这些能力本身合理，但它不是“纯 UI crate”，应在模块/组合层明确标注，最终把产品启动组合从 view/update 中抽出。
3. **Iced 与 Bevy 的页面状态模型尚未完全同源**。Iced 主要以 `AppState`/`Message` 驱动，Bevy 主要以各页私有 `Projection`/`UiCommand` 驱动；两者都能映射到部分 contract，但还没有一套覆盖 11 个页面的 shared page snapshot/event vocabulary。
4. **现有主控计划部分状态过满**。它是 225 项目标清单，不是 225 项已完成证明；Wave 和“已交付”文字必须以双端 live 行为、测试和视觉证据重新核验。

## 0.30 架构收口顺序

以下五项是 225 项功能开发前的 P0 闸门：

| 闸门 | 收口动作 | 完成证据 |
| --- | --- | --- |
| A-01 | 将 Iced 的 boot、tray、Admin、文件/进程/系统代理 glue 与页面 view/update 分成明确的 UI adapter 和 desktop composition；先允许同 crate 分目录，稳定后再决定是否拆 crate。 | UI 模块不再直接持有 desktop service 的业务状态；启动组合有单独入口测试 |
| A-02 | 在 contract/application 形成覆盖 11 个页面的 snapshot/event/capability vocabulary；Iced `Message` 与 Bevy `UiCommand` 只做 toolkit 映射。 | 同一个 intent 在两个 UI 上产生相同 typed result、error、revision/generation |
| A-03 | 为 Bevy 每一个业务页增加真实 application source；生产路由禁止调用 `Projection::demo()`，demo 只允许出现在 capture/test composition。 | 11 页均能在 live/unavailable/empty/error 状态下渲染并更新 |
| A-04 | 建立双端 parity guard，检查页面、intent、能力状态、错误状态和测试覆盖，不只检查枚举数量。 | CI 对缺页、单端命令、单端测试和 demo 泄漏 fail closed |
| A-05 | 建立“shared + Iced + Bevy + 双端测试 + 视觉/宿主证据”的统一交付模板。 | 任一项缺一个 lane 就不能标记完成 |

## 双端完成定义

225 项中的每一项都必须同时交付四层：

```text
shared contract/application behavior
        + Iced adapter/view/update
        + Bevy adapter/projection/scene
        + Iced + Bevy headless behavior tests
        + applicable desktop/mobile live or host evidence
```

状态只允许使用：

- `planned`：只有目标描述；
- `shared-ready`：契约/application 已具备，但 UI 未双端完成；
- `iced-ready` / `bevy-ready`：单端完成，仍然不能计入交付；
- `parity-ready`：双端行为、错误、能力和测试均通过；
- `host-verified`：适用宿主和打包/真实内核证据也通过。

任何只在 Iced 或只在 Bevy 中完成的项目，都必须继续显示为未完成，不能用“另一端之后补”作为里程碑完成条件。

## 页面与宿主责任矩阵

| 责任 | Iced | Bevy UI | Desktop / Android / iOS host |
| --- | --- | --- | --- |
| 页面布局、控件、手势 | Iced 本地实现 | Bevy scene/system 本地实现 | 不拥有页面状态 |
| 命令提交 | `Message` → `CommandIntent` | `UiCommand` → `CommandIntent` | 不绕过 application |
| 业务快照和结果 | 只读 projection/cache | 只读 projection/cache | 提供 port 实现 |
| Core 生命周期、订阅、同步、配置事务 | 调 application | 调 application | 实现进程、文件、权限、VPN 等 port |
| 系统代理、TUN/VPN、托盘、通知 | desktop capability | 按宿主注入 capability | 真实 owner |
| 移动前后台与 VPN 权限 | 不适用或明确 unsupported | Bevy mobile UI 只消费 capability | Android/iOS host 负责 |

## 审计门禁

本审计结论成立的静态证据：

```bash
python3 scripts/quality/core-boundary-guard.py --mode enforce
python3 scripts/quality/doc-link-guard.py --mode enforce
python3 scripts/quality/import-guard.py --mode enforce
```

这些命令只能证明底层依赖边界，没有证明双端页面已具备真实数据。因此它们必须与 live projection、双端行为测试和宿主 smoke 一起使用。

