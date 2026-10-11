# mihomo-config

> 层级：L3

Mihomo YAML 与 profile adapter：解析、校验、应用观察、配置存储和 Mihomo 专属选项编解码。

原始用户文档身份和跨引擎意图由共享 contract/application 定义；本 crate 不解释 UI 状态。Mihomo controller 边界见 [`mihomo-api` 本地文档](../mihomo-api/docs/README.md)。

Profile YAML、options、profile-state、backup、snapshot 和 WebDAV 本地状态位于共享 configs 根下的 `engines/mihomo/`。首次初始化用显式 `engine_scoped_v1` 迁移旧共享路径与 profile-state；订阅凭据 key 也含 `mihomo` namespace。生产组合必须经 `ConfigManager::initialize_profile_namespace` 或对应 ProfileStore 初始化入口后再读写。
