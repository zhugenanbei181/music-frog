# mihomo-version

> 层级：L3

管理 core 与应用版本信息、下载、摘要校验、安装、升级通道和回滚。

版本资产必须按 engine 与平台隔离；版本存在不能推导某操作受支持。

引擎标识、版本能力矩阵与目标制品清单已迁入 `mihomo-driver::version`；本 crate 只保留发行通道、下载、摘要校验、安装、升级与回滚。

`VersionManager::observe_default_binary` 用 `DefaultBinaryObservation` 只读检查当前默认版本及二进制。它优先读取 engine-scoped 配置；仅当该配置不存在时才观察旧版配置和二进制，不创建目录、不迁移文件。`get_default` 等产品操作仍按显式初始化路径工作；Doctor 修复只在明确的 `doctor fix` action 中写入。
