# singbox-config

> 层级：L3

产品侧 sing-box profile 事务：绑定 `ProfileConfigurationTransactionScope`、workspace lease 与 store CAS，把 `singbox-driver` 提供的中性 JSON codec、官方 `check` 事务钩子与精确文件读回接入共享 `ProfileStore`。

生产 `ProfileStore` 由 `infiltrator-profile-store::EngineProfileStore` 直接实现；`ConfigManager` 只负责把 sing-box codec 和文件后缀注入共享 owner。`profile_document_codec` 仅保留产品 `ProfileConfigurationCodecPort` 扩展的 newtype（orphan rule 要求产品本地类型），全部中性行为委托 `singbox_driver::codec::SingBoxProfileDocumentCodec`。本 crate 不拥有 controller API 或 binary version lifecycle。
