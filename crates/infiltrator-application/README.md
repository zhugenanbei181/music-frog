# infiltrator-application

> 层级：L3

实现共享用例、命令编排和跨端业务事务。公开接口使用 contract 类型并经 ports 调用副作用；页面快照装配属于产品组合层，profile metadata、订阅状态/配额、Mixin Studio 文案与失败文案等跨表面投影由独立 presentation crate 统一提供。Profile display name 是独立于文件身份的共享元数据，只经 ProfileApplication 与 ProfileStore owner 更新；catalog reorder 使用完整 ProfileCatalogSnapshot 做 compare 并返回 typed owner readback；clear selection 使用预期 ActiveProfileReferenceState 与 owner 授权 CAS，返回精确 Unselected readback，不停止独立 Core/VPN service。

Age 私钥变更经 ProfileApplication 提交显式 target、当前选中引用与 owner 授权，回执只有配置状态和 revision。ProfileApplication 的普通 profile read model 不包含私钥；密钥值本身使用不可格式化、drop 时清零的 contract secret 类型。

凭据按 engine/ProfileId 独立于文档存在与否，因此首次导入加密订阅时可先安全设置密钥，再由同一 ProfileApplication 解密、校验并创建 Profile；失败时不保存订阅正文，密钥保留以支持重试或显式清除。

URL 订阅导入与刷新遇到 armored Age 内容时，只请求 ProfileStore owner 在内部解密；Application 只用 redacted、drop 时清零的临时内容做 YAML 校验与 Profile commit。缺少凭据或解密失败时不保存订阅正文。

本 crate 不选择 executor、不创建具体网络客户端，也不依赖 UI。异步调度由 composition 注入；一个业务语义只能有一个 owner。

Profile 用例只依赖活动 profile applier 和显式 `Offline` / `CoreOwner` access；网络用例通过 typed `PublicIpProbeRoute` 选择直连或已配置的 HTTP proxy，不把缺失 endpoint 当成直连。

原文搜索片段、Unicode/grapheme 边界与范围重放归纯 Rust `zhuge-ui-core`，
不引入 UI toolkit。产品搜索条件与拼音匹配由本层定义。日志用例只保留有界、scope-fenced 原始行、流状态、过滤事实和原文导出；日志解析、展示行与页面状态由 presentation 投影。RoutingIntent 的纯观察/请求/失败阶段归约及共享 copy 也由 presentation 持有；application 只执行命令并返回 Core 读回事实。

`RuleListApplication` 只负责 source-fenced 规则提交与端口副作用；规则草稿、trace、statistics、日志导出、脚本导出及快照恢复工作台的 typed reducers 和共享投影由 `infiltrator-presentation` 持有。本 crate 仍负责文件、Core 命令与其他 host port 的实际副作用。

跨包 fake ports 与确定性测试数据统一由 [infiltrator-test-support](../infiltrator-test-support/README.md) 持有；Application 仅以 dev-dependency 使用它，不公开 UI capture fixtures。需要 CoreApplication 或应用快照的 capture 组装由 `infiltrator-composition` 提供。

`ResourceApplication` 编排 scoped 资源采样、内存阈值与 GC 冷却；并发采样、scope 变化、Unsupported、失败及 GC 后读回应分别保持 typed 状态。

`RuleProviderRuntimeApplication` 负责 owner/revision fence、能力 admission 与 rule-provider readback；`GeoDatabaseApplication` 只将 Core 接受的异步触发建模为 `Accepted`，不报告数据库下载完成。
