# mihomo-process

> 层级：L3

实现 Mihomo Core 的宿主进程边界：启动参数、进程归属、PID 对账、终止读回和操作系统进程保护。

共享路径、凭据、锁与 OS 能力由 `infiltrator-platform` 提供；controller 协议由 `mihomo-api` 提供。本 crate 不负责 Mihomo 配置文档或版本资产管理。
