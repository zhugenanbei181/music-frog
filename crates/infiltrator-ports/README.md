# infiltrator-ports

> 层级：L3

定义 application 使用的出站能力接口，如 core 生命周期、数据存储、网络访问、系统代理、VPN 和版本服务。

Trait 表达能力，不绑定具体 adapter、Tokio channel、HTTP response 或 UI 类型。实现由 composition 和 host adapter 注入。

Core 生命周期使用 `CoreLifecyclePort`，活动 profile 的 revision-fenced 写入使用 `ActiveProfileReferenceApplier`，HTTP proxy endpoint 使用 `RuntimeHttpProxyEndpointPort` 与 `Configured` / `NotConfigured` 状态。`RuntimeGateway` 只承载 controller 操作；公网探测路由以 `PublicIpProbeRoute` 明确区分直连和 HTTP proxy。

运行态路由意图使用专用 `RoutingIntentPort`，其成功结果必须是 adapter 写后读回的实际意图。

运行中 Core 的 Fake-IP 清理使用独立 `ControllerFakeIpCachePort`，请求和回执绑定已验证的 `CoreScope` 与操作 ID；宿主本地缓存端口不能代替 controller 操作。

Core 资源通过 `CoreResourcePort` 按 `CoreScope` 读取；支持状态是 typed outcome，GC 的 `Collected` 回执必须带同 owner 的真实 post-GC memory readback。
