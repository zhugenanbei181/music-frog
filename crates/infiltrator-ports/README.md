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

Profile catalog 顺序使用完整 `ProfileCatalogSnapshot` 作为 compare 值，`set_profile_catalog_order` 只接受该快照的精确排列，并以 Applied/Unchanged/Stale 回执返回真实读回。

活动 profile 清除在 `ProfileApplication` 使用 revisioned active-reference port CAS；成功结果要求精确的 `Unselected` readback，授权可来自无 owner 状态或匹配的 bound Core owner。清除选择本身不触发 Core/VPN 停止。

Age 凭据通过 `ProfileStore` 的 owner-authorized read/mutation port 编辑：请求同时绑定预期 selection、Core owner 授权与凭据 revision；读回只包含 engine/profile、revision 和配置状态，不返回私钥。凭据可以先于 Profile 文档设置，供首次加密订阅导入解密；订阅正文通过同一 owner 解密后才创建文档。

URL 导入/刷新把 armored Age 内容交给 ProfileStore 的 Core 内解密 port；私钥不返回给调用方，解密文本使用临时 zeroizing 类型。Profile 删除会将凭据改为递增 revision 的 `NotConfigured` tombstone，避免同一 ProfileId 重建后旧凭据或旧 compare 被复用。
