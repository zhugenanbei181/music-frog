# infiltrator-composition

> 层级：L3

组合 application 与具体 outbound/host adapter，建立产品运行时和数据流。通用 engine registry 支持 Mihomo provider 与 sing-box provider；每个产品 root 显式选择 provider，并在 Core actor 建立后绑定 typed engine-operation scope 与配置事务 owner。当前桌面和 Android 产品 root 仍选择 Mihomo。

业务规则留在 application；UI 不得自行创建 engine client、backend provider 或第二套命令服务。缺失的 owner binding 和未验证能力保持 typed `Unsupported`。
