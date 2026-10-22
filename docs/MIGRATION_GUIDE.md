# Lurker 中立内核接入与架构迁移指南

> 层级：L2

本文详细指导开发者如何将基于旧版单体客户端、私有适配层或传统多引擎旁路封装的代码库，平滑迁移到 Lurker 中立内核与单向通信管道体系。

---

## 一、迁移背景与收益

在传统或旧版实现中，UI 表面往往直接持有配置管理器句柄、直接调用引擎进程启动/停止命令、或者在本地比对代理节点 ID 来推导选中的节点。这种模式存在三大痛点：
1. **多头真源冲突**：UI 端维护的内部状态与内核真实路由状态发生漂移；
2. **多引擎耦合**：切换底层代理引擎（如从 Mihomo 切换到 sing-box）需要改动所有上层界面代码；
3. **难以进行自动化测试**：UI 强绑定平台和图形上下文，无法在 CI 环境中进行确定性无头回归。

迁移到 Lurker 中立架构后：
- **内核驱动完全解耦**：上层仅依赖 `lurker-core-contract`，底层可在不同驱动间透明切换；
- **全链路强类型契约**：所有状态变更均收敛为只读 `SurfaceSnapshot` 与强类型 `CommandIntent`；
- **极速无头测试**：可使用 `RecordingCommandSink` 或 Fake 适配器在数毫秒内完成全业务流程验收。

---

## 二、三步平滑迁移路线图

### 第一步：剥离 UI 层的业务副作用与私有推导
**旧代码模式（禁止）：**
```rust
// 错误示范：UI 直接触发底层进程并本地维护状态
impl MyWindow {
    fn on_proxy_clicked(&mut self, proxy_id: String) {
        self.active_proxy = proxy_id.clone(); // 私有状态真源！
        mihomo_api::select_proxy(&proxy_id);   // 直调底层副作用！
    }
}
```

**新代码模式（标准）：**
```rust
// 正确示范：通过 CommandSink 发送强类型意图
impl MyWindow {
    fn on_proxy_clicked(&self, proxy_id: String) {
        let sink = self.command_sink.clone();
        tokio::spawn(async move {
            let _ = sink.submit(CommandIntent::SelectOutboundGroupProxy {
                group_name: "GLOBAL".to_string(),
                proxy_name: proxy_id,
            }).await;
        });
    }
}
```

### 第二步：由 `SurfaceReader` 权威泵送只读快照
将页面属性、开关状态与流量数据全部改造为订阅式：
1. 实现监听事件循环（或各 UI 框架的响应式订阅，如 Iced 的 `Subscription` 或 Bevy 的 `System`）；
2. 消费由 `SurfaceReader` 提供的权威数据快照，纯被动刷新界面；
3. 本地仅保留短暂的用户输入草稿（如正在输入的搜索框文本），不保留业务决策结果。

### 第三步：依赖项替换与 Cargo.toml 清理
从业务 crate 的 `Cargo.toml` 中移除对私有驱动的直接依赖，统一替换为 Lurker 中性三件套：
```toml
[dependencies]
# 核心契约与抽象端口（完全中立）
lurker-core-contract = { version = "0.68.0", path = "../../crates/lurker-core-contract" }
lurker-core-ports = { version = "0.68.0", path = "../../crates/lurker-core-ports" }
infiltrator-ports = { version = "0.68.0", path = "../../crates/infiltrator-ports" }
```

---

## 三、兼容性检查清单

- [ ] UI 层已搜索并清理所有 `mihomo_*` / `singbox_*` 的直接函数调用；
- [ ] 所有多阶段操作均改为匹配显式 typed enum，无 `bool` 组合隐式状态机；
- [ ] 单文件源码行数严格受控在 800 行之内（Slint UI 限制在 600 行之内）；
- [ ] 运行全套门禁通过无报错。
