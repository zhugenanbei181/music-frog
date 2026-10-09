# 双端交付模板（0.30）

> 层级：终端（已归档，冻结）：现行规则见 `docs/archive/README.md` 登记的取代者；本页链接可能指向迁移前位置。

任何共享功能必须按一个 `DUAL-组号-项号` 交付。这个模板是完成定义，不是建议清单。

交付必须同时满足 [UI_PARITY_AUDIT.md](UI_PARITY_AUDIT.md) 的 L1/L2/L3。登记 `FeatureId`、两端精确 nextest ID、适用状态机分支和标准/紧凑像素回执；只拥有 headless 测试或源码名称不能关闭条目。

## 功能登记

- ID：`DUAL-__-__`
- 场景身份：`FeatureId` / Token
- 两端 `feature_evidence.tsv` 精确测试 ID：
- 成功 / 取消 / 关闭 / 错误 / 重试 / 空态 / typed 不支持分支：
- 两端 1180×780 与 720×480 新鲜像素回执：
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
