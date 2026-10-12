# singbox-api

> 层级：L3

实现 sing-box Clash API adapter：HTTP/wire 类型、controller 配置、`/version` readiness、outbound/routing observation 与写入，以及逐 operation 能力证据。

本 crate 只声明真实验证过的 controller 行为；其余操作返回 typed `Unsupported`。Core 进程、原生配置和版本资产由独立职责 crate 持有。
