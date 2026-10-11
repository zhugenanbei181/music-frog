# infiltrator-http

> 层级：L3

统一 HTTP client 依赖和平台 TLS feature，供 workspace adapter 使用。

只负责 transport 配置，不承载 Mihomo endpoint、业务重试或用户状态；协议语义归调用方的 adapter。
