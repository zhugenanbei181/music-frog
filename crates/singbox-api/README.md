# singbox-api

> 层级：L3

产品侧 sing-box backend capability 声明：从打包证据锁与逐 operation 声明文件构建 `CoreCapabilityProvider`，为产品组合根提供该引擎的可验证能力矩阵。

引擎实现（Clash API 客户端、controller preflight、readiness、outbound/routing adapter、进程监督、JSON codec、版本探测）已迁入中性 `singbox-driver`；本 crate 不保留实现副本，也不 re-export driver 类型。本 crate 只声明真实验证过的 controller 行为；其余操作返回 typed `Unsupported`。
