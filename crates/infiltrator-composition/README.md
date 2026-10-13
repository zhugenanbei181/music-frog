# infiltrator-composition

> 层级：L3

组合 application 与具体 outbound/host adapter，建立产品运行时和数据流。`ProductSurfaceReader` 在此实现跨页面 `SurfaceReader` port，汇集 application 用例的观测并组装 contract 快照，纯 Doctor 页面折叠调用 presentation；application 不持有页面快照 assembler。Mihomo 与 sing-box 都实现 `infiltrator-ports::EngineBackendProvider`，并生成相同的 `EngineBackendPorts`；engine-specific process、native config、controller API 和 version owner 由组合根装配。每个产品 root 显式选择 provider，并在 Core actor 建立后绑定 typed engine-operation scope 与配置事务 owner。Desktop 产品按持久化 active-profile reference 选择双引擎；Android `:vpn` 现按同一引用 dispatch Mihomo 或 sing-box Core lifecycle。Android `:vpn` sing-box Core lifecycle 与 typed version-management owner operations 已接入；running runtime operations 和 app UI 版本安装/激活入口仍未接入。

业务规则留在 application；UI 不得自行创建 engine client、backend provider 或第二套命令服务。缺失的 owner binding 和未验证能力保持 typed `Unsupported`。

Production-provider conformance 已验证 Mihomo 与 sing-box 的 engine/instance identity 绑定及 typed profile transaction 行为。Focused fake suite run `91abed5f-ac17-4006-844e-f09a590e037e` 与 SHA-locked real-process suite run `a7e148c8-4bd5-48b5-9560-271d01ec88e4` 均为 2/2。此结果验证 provider contract，不代替具体产品根的目标版本、进程所有权和设备验收；未接入的 sing-box operation 仍 fail closed。

sing-box 生产配置通过 `ConfigManager::with_profile_codec_and_store` 注入 JSON codec，再把其共享 `EngineProfileStore` 交给 provider native transaction；通用文件与 `ProfileStore` 实现归 `infiltrator-profile-store`，`ConfigManager` 不实现该共享 port。组合 conformance 使用真实 engine-scoped 文件存储与共享 active-reference CAS、fake SecureStore 和 fake CLI validator，验证 Core engine/instance identity、workspace lease、原始 JSON/sidecar 与 native config 的逐字节读回；Core 保持 Stopped。Android 产品路径另由 Android owner fake/JVM 与 Rust nextest 覆盖，仍缺 Android target、APK 和设备进程验收。
