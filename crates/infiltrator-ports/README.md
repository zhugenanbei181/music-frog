# infiltrator-ports

> 层级：L3

定义 application 使用的出站能力接口，如 core 生命周期、数据存储、网络访问、系统代理、VPN 和版本服务。

Trait 表达能力，不绑定具体 adapter、Tokio channel、HTTP response 或 UI 类型。实现由 composition 和 host adapter 注入。

Core 生命周期使用 `CoreLifecyclePort`；活动 profile 的 revision-fenced 应用由独立的 `ActiveProfileReferenceApplier` 承担；owner scope 通过 `CoreScopeSource` 以 `Unavailable` / `Observed(CoreScope)` 显式表达；HTTP proxy endpoint 使用 `RuntimeHttpProxyEndpointPort` 与 `Configured` / `NotConfigured` 状态。Controller configuration、connection、stream、resource、provider 和 Geo 能力分别由 typed ports 注入。公网探测路由以 `PublicIpProbeRoute` 明确区分直连和 HTTP proxy。

运行态路由意图使用专用 `RoutingIntentPort`，其成功结果必须是 adapter 写后读回的实际意图。

运行中 Core 的 Fake-IP 清理使用独立 `ControllerFakeIpCachePort`，请求和回执绑定已验证的 `CoreScope` 与操作 ID；宿主本地缓存端口不能代替 controller 操作。

Core 资源通过 `CoreResourcePort` 按 `CoreScope` 读取；支持状态是 typed outcome，GC 的 `Collected` 回执必须带同 owner 的真实 post-GC memory readback。

Rule provider 的 inventory/payload/refresh 使用 `RuleProviderPort`，并以 engine-scoped ID、`CoreScope` 和 inventory revision 拒绝 foreign/stale 操作。Geo 数据库触发使用 `GeoDatabasePort`，只报告 controller 接受了异步请求。

Profile 展示名通过 `ProfileStore` 的 typed observation/mutation 与 `ProfileApplication` 访问；它独立于稳定 `ProfileId` 和文件名持久化，也不触发 Core/VPN 生命周期变化。
