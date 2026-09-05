# 双端交付模板（0.30）

任何共享功能必须按一个 `DUAL-组号-项号` 交付。这个模板是完成定义，不是建议清单。

## 功能登记

- ID：`DUAL-__-__`
- 用户意图：
- Mihomo 配置/API 边界：
- `shared/application` owner：
- 关联能力：
- 关联 revision/generation：

## 两条 UI lane

### Iced

- [ ] Message → shared `CommandIntent` 映射
- [ ] shared snapshot/event → Iced projection
- [ ] loading / empty / unavailable / failed / success 状态
- [ ] headless 行为测试
- [ ] 桌面视觉或真实 host evidence（适用时）

### Bevy

- [ ] `UiCommand` → shared `CommandIntent` 映射
- [ ] shared snapshot/event → Bevy projection/scene
- [ ] loading / empty / unavailable / failed / success 状态
- [ ] headless 行为测试
- [ ] 桌面、Android 或 iOS 宿主证据（适用时）

## 共同验收

- [ ] 两端使用同一个 typed result、error、capability 和 generation 语义
- [ ] 没有页面直接构造 Mihomo client、Reqwest、Tokio channel 或文件实现
- [ ] 没有把 `demo` 数据作为生产 live 数据
- [ ] parity guard 通过
- [ ] `parity-ready`：只有两条 UI lane 和所有适用证据都通过后才能标记

单端完成只能记录为 `iced-ready` 或 `bevy-ready`，不能计入版本完成度。
