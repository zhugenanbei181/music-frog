# infiltrator-core

> 层级：L3

承载 core 周边的具体服务与 adapter：启动装配、配置读写与应用、诊断、缓存、同步和系统能力接线。

共享用户语义归 application/domain；本 crate 不承载 UI 状态，不被界面直接用作代理内核（Mihomo / sing-box）协议入口。
