# 架构边界

> 层级：L2

Rust 是 Core、配置和业务编排的唯一边界。跨引擎契约见 [CORE_BACKENDS.md](CORE_BACKENDS.md)，各 crate 职责由其包内说明承载。

```text
contract → domain ────────────────┐
shared ───────────────────────────┼→ presentation → Iced / Bevy / Slint UI
contract + domain + ports ────────┼→ application → composition → host/core adapters
```

- `contract` 定义跨端意图、结果与快照；`domain` 定义业务规则；`ports` 定义副作用接口；`application` 实现用例；`presentation` 只折叠已观测快照供多表面复用，不拥有事实或副作用。
- `composition` 注入具体 adapter，并把 application 观测组装为跨页面 contract 快照；`ProductSurfaceReader` 在此实现 `SurfaceReader` port，页面快照需要纯展示折叠时调用 presentation。共享 OS 能力归 `infiltrator-platform`；engine-neutral 持久化内核（durable replace、跨进程写锁、on-disk layout、settings/profile-state schema、原子 document I/O 与 `ProfileStoreCore`）由 [`lurker-profile-store`](../crates/lurker-profile-store/README.md) 持有，产品私有部分（Age/订阅凭据、workspace、catalog、owner record、active-reference CAS 与 `ProfileStore` port 实现）由 `infiltrator-profile-store::EngineProfileStore` 在该内核之上实现并注入产品 layout。`mihomo-config::ConfigManager` 只保留 Mihomo YAML、legacy migration 与 engine facade，不实现共享 persistence port。各 engine 的中性实现（controller client、readiness、typed outbound/routing、原生 document codec、进程监督）收敛在 [`mihomo-driver`](../crates/mihomo-driver/README.md) / [`singbox-driver`](../crates/singbox-driver/README.md)，产品侧只保留 operation adapter（rule provider、geo database、runtime configuration、capability evidence）与版本资产。准确 owner 与依赖方向见 [Core 后端契约](CORE_BACKENDS.md)。
- 引擎 backend 的中性词汇（bundle、provider、availability、owner-scope binding 与 outbound/routing 操作）由 [`lurker-core-ports::engine_backend`](../crates/lurker-core-ports/src/engine_backend.rs) 唯一持有；产品扩展 [`infiltrator-ports::engine_backend`](../crates/infiltrator-ports/src/engine_backend.rs) 只增加 native configuration transaction 与 rule-provider/geo 操作。[`lurker-engine-runtime::registry`](../crates/lurker-engine-runtime/README.md) 按 `EngineId` 分派 provider 并校验返回 bundle 身份，不依赖任何 driver、产品 crate 或 UI toolkit；具体 driver 由使用者组合。
- Core 生命周期、活动 profile 写入、controller 操作、proxy endpoint 观察和公网探测路由分别由窄 port 表达；一个宽泛 runtime trait 不承担多个 owner 的组合职责。
- 依赖向外流动。共享层不依赖 runtime、网络、OS 或 UI toolkit；UI 不启动 core、不写 canonical 配置、不创建第二个业务 owner。
- Bevy desktop 的 `DesktopProductSession` 组合、Core surface pump、命令 adapter 和进程入口由 `infiltrator-bevy-desktop-host` 持有；`infiltrator-bevy-ui` 只消费注入的 typed surface、command sink 与宿主能力。
- 有副作用的操作由单一 owner 串行执行并校验 scope/revision/generation；迟到结果不得覆盖新状态。成功须有真实终态或读回。
- 可观察业务事实只有一个 owner；前端仅保存输入草稿和呈现缓存。
- 跨表面的纯文案、状态展示折叠和快照到视图模型映射完全由 `infiltrator-presentation` 持有，禁止在各 UI 表面复制推导逻辑。所有纯呈现模块均已收敛至 presentation，并在统一批次中完成验证。
- 不触碰产品 contract 的呈现内核由 [`lurker-ui-presentation`](../crates/lurker-ui-presentation/README.md) 持有：typed reducer 阶段（RoutingIntent 改变阶段、代理分组顺序草稿/回执、DNS 泄漏探测阶段）、日志视口跟随策略，以及本地化 key 端口 `TextCatalog` 与占位符插值规则。该 crate 只依赖 `lurker-core-contract` 与 `std`，可发布并供任意表面复用；**key 属于中性内核，文案与 catalog 数据属于各产品**（`infiltrator-shared` 为自家 `Lang` 实现 `TextCatalog` 并持有 gettext 资源）。仍依赖产品 contract 的投影继续留在 `infiltrator-presentation`，不得把文案下沉进中性内核。
- “共享一个 Core”指共享 engine-neutral use case、身份/能力/命令/快照契约，不代表跨 Android 进程共享 Rust 静态变量。Android 中只有 `:vpn` 能创建 Core application、配置事务、VPN 和内核进程；Compose/Bevy 通过 typed IPC 访问 owner。桌面可在本机进程内组合 Core 与 adapter，但仍只允许一个活动 Core actor。
- Core 必须允许每个后端使用各自独立的进程、原生配置格式、controller API 和版本资产 adapter；公共层只表达经验证的共同意图。`Unsupported` 属于对应 engine adapter 的逐操作能力，不改变职责拆分，细则见 [CORE_BACKENDS.md](CORE_BACKENDS.md)。

## 扩展边界（新增引擎 / 平台 / UI 各自要做什么）

| 扩展 | 必须实现 / 提供 | 不得修改或复制 |
| --- | --- | --- |
| 新引擎 | `CoreProcess`、`CoreReadiness`、`OutboundPort`、`RoutingIntentPort`、`ProfileDocumentCodecPort`，并提供 `EngineBackendProvider<EngineBackendPorts>` | 中性 contract/ports/store/runtime；不得把产品语义写进中立层 |
| 新平台 | `StorePlatform`（原子替换、写锁、实例锁、目录/文件原语）；driver 内的 `CoreProcess` 进程监督；宿主组合 | `PURE_NEUTRAL` crate（contract/ports/ui-presentation）不得出现 `cfg(windows/unix)`、`libc`、`windows_sys` |
| 新 UI | 消费 `SurfaceReader` 快照、发送 typed 意图、实现 `TextCatalog` 取文案 key | 不得直连 Core 副作用、不得自建业务真源、不得复制 presentation 的推导逻辑 |

`CommandSink`（统一意图通道）属 PUB-004.2，尚未落地；当前各表面通过既有 typed command adapter 发送命令。扩展路径的门禁由 `lurker-neutrality-guard.py`、`core-boundary-guard.py` 与 `publish-order-guard.py` 共同约束。

## 架构未决处置与自底向上演进铁律（底层先决，严禁上层修补）

本铁律适用于全仓所有 Core crate、适配层与所有 UI 表面（Iced、Bevy、Slint、Android Compose），全员强制遵照执行。

### 核心禁令：严禁上层修修补补

当遇到业务场景复杂、底层模型缺失、生命周期阶段模糊、跨端契约不一致或 Owner 权责未决时，**严禁在上层（UI 视图/状态、Presentation、Host 组装层）就地打补丁修补**。

**典型上层修补反模式（禁止行为）：**
1. **UI 自行推导业务事实（UI-Derived Truth）**：UI 表面通过本地比对字符串、集合成员或 ID 等，私自猜测如活动状态、运行状态等核心业务事实，而非由底层 Owner 权威读回；
2. **标志位与布尔修补（Flag Patching）**：为应付未决的异步操作阶段，在 UI 状态或事件中增加 `is_loading`、`is_refreshing`、`is_saving` 等散乱布尔，藏匿隐式状态机；
3. **上层容错掩盖底层缺陷（Masking Glue）**：在 Presenter、View 或 Host 中使用 `unwrap_or_default()`、静默 fallback、软编码降级或忽略错误，掩盖底层端口或适配器未实现的真相；
4. **UI 空头支票（UI-First Empty Shell）**：界面交互与按钮已经先行上线，但底层调用依然是空实现（no-op）、mock 或带有 `TODO: 待 Core 实现` 注释；
5. **表面分裂修补（Surface Divergence）**：仅在某一特定 UI（如 Iced）中打补丁解决问题，导致同一契约在 Bevy、Android 或其它表面产生行为分歧与状态不一致。

### 自底向上四阶推进模型（Bottom-Up Resolution Ladder）

任何涉及新功能开发、重构或缺陷修复时，若架构契约未清晰裁决，必须立即暂停上层工作，按以下四阶由底至上逐步解决：

```text
[阶梯 1: 契约与领域] Contract / Domain / Ports / Owner
       ↓ 裁决单一权威所有者、显式状态机、typed 意图与权威读回（Readback）
[阶梯 2: 内核与适配] Core Application / Ports / Adapters
       ↓ 完整业务流程闭环，编写 fake/mock/nextest 严格通过单元与集成验收
[阶梯 3: 呈现与装配] infiltrator-presentation / Host Composition
       ↓ 跨表面纯展示投影折叠，宿主环境注入稳定 typed 依赖，不留私有业务逻辑
[阶梯 4: 消费端表面] UI Surfaces: Iced / Bevy / Slint / Android
         纯粹被动消费底层读回快照并提交 typed 意图，无任何私有业务真源
```

1. **阶梯 1：底层契约与领域裁决（L2 Contract / Domain）**
   - 确定唯一权威 Owner（Single Source of Truth）；
   - 契约必须使用显式 `enum` 表达所有合法互斥状态与生命周期阶段；
   - 定义完整的输入意图（Intent/Request）、执行结果（Outcome）与权威读回（Readback）；
   - 拒绝多布尔组合、拒绝由前端派生状态反向推测底层状态。
2. **阶梯 2：内核应用与适配收口（L3 Core Application / Adapters）**
   - 在应用层与各 backend adapter（Mihomo/sing-box、Android VPNService/IPC）中实现具体业务逻辑；
   - 保证操作成功时具有真实终态或精确读回，失败时返回具体 typed 错误原因；
   - 必须通过 fake/nextest/conformance 严格验收，证明底层契约闭环可靠。
3. **阶梯 3：跨端呈现与装配注入（L3 Presentation / Host Composition）**
   - 跨表面复用的纯展示映射与文本折叠统一归入 `infiltrator-presentation`；
   - 宿主装配层负责将底层端口绑定到消费端，不得在装配层残留私有状态或私设流程分支。
4. **阶梯 4：UI 消费端接入（L3 UI Surfaces）**
   - 所有 UI 表面（Iced、Bevy、Slint、Android）作为纯被动消费端，直接消费阶梯 3 产出的视图模型或底层快照；
   - UI 交互直接映射为阶梯 1 的 typed 意图命令，严禁在 UI 层自行处理业务副作用或模拟操作结果；
   - 底层未收口前，上层严禁提前合入。

### 批次切片与组团验收流水线机制（高效率工程推进模式）

为了在执行四阶自底向上演进时不产生巨大的集成摩擦和漫长的测试等待开销，全仓强制实施“改动成批推进、验收组团收口”的高效推进模式：
1. **批次切片（Batching by Cohesive Slice）**：改动严格按照阶梯分批进行。一个批次只推进一个清晰的职责切片（例如：单组契约演进、单一应用用例闭环、或一组强内聚的 presentation 投影迁移）。同批内改动一并完成单元测试与废弃符号清零，同次提交，禁止跨阶梯东拼西凑；
2. **组团集中验收（Grouped Verification Batches）**：切片内部只使用定向测试与局部守卫进行秒级反馈；当一批相关的切片完成后，汇总为组团统一验收批次，一次性执行全仓回归、结构门禁与跨表面验证，一次性批量解锁并办结多个关联条目，避免单个小改动频繁打断工程流。

Iced、Bevy 与 Slint 的共享边界和验收状态见 [UI 契约](../crates/infiltrator-contract/docs/README.md)。Android owner 与生命周期见 [Android 文档](../android/docs/README.md)；Bevy ECS 规则见 [Bevy UI 文档](../crates/infiltrator-bevy-ui/docs/README.md)。
