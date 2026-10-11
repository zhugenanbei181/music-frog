# infiltrator-composition

> 层级：L3

组合 application 与具体 outbound/host adapter，建立产品运行时和数据流。通用 engine registry 支持 Mihomo provider 与 sing-box provider；每个产品 root 显式选择 provider，并在 Core actor 建立后绑定 typed engine-operation scope 与配置事务 owner。当前桌面和 Android 产品 root 仍选择 Mihomo。

业务规则留在 application；UI 不得自行创建 engine client、backend provider 或第二套命令服务。缺失的 owner binding 和未验证能力保持 typed `Unsupported`。

Production-provider conformance 已验证 Mihomo 与 sing-box 的 engine/instance identity 绑定及 typed profile transaction 行为。Focused fake suite run `91abed5f-ac17-4006-844e-f09a590e037e` 与 SHA-locked real-process suite run `a7e148c8-4bd5-48b5-9560-271d01ec88e4` 均为 2/2。此结果验证 provider contract，不代表桌面或 Android 产品根已选择 sing-box；未选择的产品能力仍 fail closed。
