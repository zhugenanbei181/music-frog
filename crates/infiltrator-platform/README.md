# infiltrator-platform

> 层级：L3

共享 OS 与宿主能力 adapter：home/config 路径、凭据存储、安全文件替换、跨进程锁、Android Core host bridge、网络接口与电源观察。

本 crate 不拥有 Mihomo 或 sing-box 的原生协议、配置格式、Core 命令参数、版本资产或业务策略。引擎专属进程分别由对应的 process crate 实现。
