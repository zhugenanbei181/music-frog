# infiltrator-composition

> 层级：L3

组合 application 与具体 outbound/host adapter，建立产品运行时和数据流。当前 Mihomo adapter 的选择与注入属于本层。

业务规则留在 application；UI 不得自行创建 `MihomoClient`、runtime 或第二套命令服务。
