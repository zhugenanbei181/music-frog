# infiltrator-composition

> 层级：L3

组合 application 与具体 outbound/host adapter，建立产品运行时和数据流。通用 engine registry 支持 Mihomo provider 与 sing-box provider；每个产品 root 显式选择 provider，并在 Core actor 建立后绑定 typed engine-operation scope 与配置事务 owner。当前桌面和 Android 产品 root 仍选择 Mihomo。

业务规则留在 application；UI 不得自行创建 engine client、backend provider 或第二套命令服务。缺失的 owner binding 和未验证能力保持 typed `Unsupported`。

Production-provider conformance 已验证 Mihomo 与 sing-box 的 engine/instance identity 绑定及 typed profile transaction 行为。Focused fake suite run `91abed5f-ac17-4006-844e-f09a590e037e` 与 SHA-locked real-process suite run `a7e148c8-4bd5-48b5-9560-271d01ec88e4` 均为 2/2。此结果验证 provider contract，不代表桌面或 Android 产品根已选择 sing-box；未选择的产品能力仍 fail closed。

sing-box 生产 ProfileStore 通过 `ConfigManager::with_profile_codec_and_store` 注入 sing-box JSON codec，并绑定到 provider 的 native transaction。组合 conformance 使用真实 engine-scoped 文件存储与共享 active-reference CAS、fake SecureStore 和 fake CLI validator，验证 Core engine/instance identity、workspace lease、原始 JSON/sidecar 与 native config 的逐字节读回；Core 保持 Stopped。composition、mihomo-config、singbox-adapter nextest run `4d667a0a-4b33-44e6-8003-a2c875f2011d` 为 219 passed、1 skipped。该测试不启动真实 sing-box 进程，也不表示 Desktop/Android 产品根已接入 sing-box。
