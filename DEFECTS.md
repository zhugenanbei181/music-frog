# 功能差距视图

> **平权证据重置**：历史“99.1%”“全量闭环”等结论不能作为当前 L2/L3 平权依据。旧字符串 guard 已删除，225 项能力按 [UI_PARITY_AUDIT.md](docs/UI_PARITY_AUDIT.md) 重新验收；当前缺口由 `scripts/parity/` 的结构化清单与实际测试/像素解析报告给出。

本文件是差距和证据索引，不是执行顺序。执行顺序、owner 和验收条件统一放在本地 `TODO.md`；功能归属见 [docs/FUNCTIONAL_MAP.md](docs/FUNCTIONAL_MAP.md)。

Bevy Android 的平台缺口按 [平台矩阵](docs/PLATFORM_MATRIX.md) 记录；保留扩展与补全契约见 [Android 产品规范](docs/android/BEVY_ANDROID_PRODUCT.md)，本地 TODO 的 `BANDROID-001`～`BANDROID-017` 负责实施与验收，历史组件完成记录不替代当前宿主、APK 和设备证据。

> 状态依据当前工作树的代码盘点。`已补齐` 只表示入口或主要实现已经出现，不等于完成了跨平台行为、真实 mihomo 和发布验证。

## 当前发布阻断

当前证据与迭代入口见 [双端审计](docs/UI_PARITY_AUDIT.md#现行证据与迭代入口)，执行顺序仍归属本地 `TODO.md`。

- 场景注册表的其余交互仍需真实双端状态机与两种视口验收；应用分流、语言选择和测速参数也必须覆盖能力并集。
- 全仓本地化门禁仍阻断遗留裸中文；原位语言回放不能被静态翻译标签替代。
- Bevy 生产 World/App world 入口及 59 处查询/参数 lint 豁免已撤，受限启动资产、原生查询和概览排除/恢复已有行为证据。像素发现的紧凑测速区域溢出与浅色低对比度已修复，浅色令牌同步共享契约及双端；仍不能外推为全部场景、文案与平台已经闭合。
- 遗留配置编辑仍有真实事务与草稿缺口：Hosts 等表面不能把队列提交当作应用成功，配置读取失败不能覆盖用户草稿；按场景验证取消、保存失败与重试。
- Iced 遗留配置/管理/运行态操作需核对组合宿主、命令和 reader 的所有权；Bevy Android 产品宿主仍待组合。
- 控制器观测已独立于偏好页，可缺失字段保留存在性，设置表单消费同一可选观测。模式动作已关联请求身份、代次与读写边界，两端保留错误操作面与适用重试；实际读回和原生点击有行为回归。专用运行控制/模式场景的完整原生布局与像素尚需闭合，不能据局部测试宣布控制流程完成。
- 最终平台包、远端完整验收工作流与公开发布尚未完成，模板夹具打包不算正式版本。
- MRS 观测仍含伪事实：`MrsAccelerationApplication` 按规则数估算文件/内存字节，并固定格式版本、校验通过和 mmap 开启；原生格式缺失时也有 Domain/HTTP 兜底。必须改为真实读取或明确未观测，不能将已有 mock 测试视为真实加速证明。
- 版本验证回执仍由 `MihomoVersionPort::LAST_VERIFICATION` 在进程内共享，命令与 reader 又各自创建版本服务；需改为产品实例持有、组合根共享，独立产品和来源切换不得借用另一份验证结果。下载取消的类型化整改不证明该所有权问题已解决。
- i18n 已扩大到 Iced update/state，但遗留管理通知、MRS 辅助文案、桌面订阅 OS 通知与其它后端业务字符串仍需迁移；已渲染的异步通知和错误面还需核对语言切换，不能以结构门零违规宣布全量运行态本地化完成。
- 两端壳层计数、活动配置、配额、速率和波形已回放共享真实观测；未知、零值与失效分开，移除了固定示例数字。模式与系统切换的未知、等待、失败和重试分支仍需核对，不能把局部业务场景回执外推为整个页面事实一致。规则稳定行身份与未提交表单的源绑定已有双端行为回归；紧凑构建器类型按钮已换行，但向导字段完整可达性及源变化表面的专用像素证据仍需闭合。

## 差距列表

| ID | 严重度 | 当前判断 | 证据 | 后续任务 |
| --- | --- | --- | --- | --- |
| D-001 | High | 已补齐入口，待控制平面收敛 | `crates/infiltrator-admin/src/admin_api.rs` 已有 runtime connections/logs/traffic/memory/IP/delay 路由 | CORE-001/003、FUNC-002 |
| D-002 | High | 已补齐入口，待跨 UI 平价 | `webui/config-manager-ui/src/App.vue` 已挂载 RuntimePanel | FUNC-002、UI-003 |
| D-003 | Medium | 已补齐入口，待统一结果语义 | `mihomo-api::test_delay`、Admin API runtime delay 路由和 RuntimePanel 均存在 | FUNC-002、QA-001 |
| D-004 | Medium | 已补齐主要流程，待交付验证 | Admin API 已有 core versions/latest/download/update/activate 路由，Iced 也有 core update state | CORE-006、QA-004 |
| D-005 | Medium | 部分完成 | `infiltrator-core` 和 UI 已有 DNS/Fake-IP/TUN/rules/providers/sniffer 的结构化/JSON 路径 | FUNC-003、CORE-005 |
| D-006 | High | 主要入口已补齐，待 shared contract | Android profiles 已有 create/select/save/delete、local import 和 subscription settings 路径 | FUNC-001、UI-004 |
| D-007 | High | Rust FFI 已成为主要来源，仍需 canonical 审计 | `AppRoutingViewModel` 通过 `appRoutingLoad/SetMode/TogglePackage`，Rust 侧有 `app_routing_*` | PLAT-002、UI-004 |
| D-008 | Medium | 已补齐入口，待回归 | Android `App.kt` 已路由 Connections，UniFFI 已提供 list/close | FUNC-002、QA-001 |
| D-009 | Medium | 已补齐入口，待与桌面语义对齐 | Android Overview 已包含 rule/global/direct/script 四种模式 | FUNC-002、UI-004 |
| D-010 | Medium | 部分完成 | Android 已暴露 `fallback-filter`、`stack`、`auto-detect-interface` 等字段，但完整字段矩阵仍未建立 | FUNC-003、CORE-005 |
| D-011 | Low | 已补齐入口，待错误/网络策略审计 | Android Overview 已调用 `ipCheck`；Rust 实现仍需纳入统一诊断契约 | FUNC-002、QA-001 |
| D-012 | High | 持续开放 | 现有单元/API 测试较多，但缺少 core version × platform × UI 的统一矩阵 | QA-001/002/004 |

## 使用原则

1. 先处理 `CORE-*`、`QA-*` 和会阻塞多端的 `FUNC-*`，不要按旧的 A/B 编号继续扩张。
2. 差距状态必须由代码、行为测试和适用平台证据共同决定；不能因为页面出现就标记完成。
3. 新发现先写入对应功能域的 TODO，再在本表增加证据；本表不保存临时实现流水。
