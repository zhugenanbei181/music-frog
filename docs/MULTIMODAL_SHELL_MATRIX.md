# 多模态外壳无头回归矩阵（组 15）

本文件是组 15（多模态外壳与极客命令流）的**可机检回归矩阵**：每一行把一项
需求绑定到真实存在、可运行的 evidence 标记，格式为 `路径::测试名`。
`scripts/quality/multimodal-shell-guard.py` 会逐行解析本表并验证：

1. 15 项全部在表内，且状态取自组 15 的统一词表（`parity-ready` /
   `shared-ready` / `planned`）；
2. 每个 evidence 标记引用的文件真实存在，且该测试名确实出现在文件里
   （防止「文档宣称有测试」）；
3. 标注为 `parity-ready` 的项必须同时给出共享/应用、Iced 与 Bevy 三侧
   evidence（少于三个标记即失败）；
4. 每行状态必须与 `docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md` 的组 15 逐项
   账目完全一致（文档之间不允许各说各话）。

矩阵只收录已经闭环或已如实标注阻断的项；`planned` 行不附 evidence，
其「诚实偏差」列说明缺什么宿主事实。

| 项 | 状态 | 共享 / 应用 evidence | Iced evidence | Bevy evidence | 诚实偏差 / 阻断 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `DUAL-15-01` | `parity-ready`（外链） | `crates/infiltrator-contract/src/responsive_viewport.rs::test_authoritative_thresholds_are_stable` | `crates/infiltrator-iced/tests/gui/responsive_elasticity_tests.rs::window_resize_updates_shared_viewport_tier` | `crates/infiltrator-bevy-ui/tests/headless/responsive_ui_tests.rs::test_bevy_breakpoint_mirrors_shared_contract_at_boundaries` | 权威台账为 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md)，本表只登记指针 |
| `DUAL-15-02` | `planned` | — | — | — | 托盘速率徽标需宿主逐采样推送；`infiltrator-desktop/src/tray_badge.rs::generate_activity_tooltip` 仍为零引用，Bevy 无托盘面 |
| `DUAL-15-03` | `parity-ready` | `crates/infiltrator-contract/src/mini_hud.rs::the_waveform_strip_projects_the_newest_real_samples` | `crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs::the_mini_hud_read_model_comes_from_live_projections` | `crates/infiltrator-bevy-ui/tests/headless/mini_hud_tests.rs::the_mounted_waveform_slots_rasterize_the_shared_strip` | 双端同源读模型：双向波形条（共享 `MiniHudWaveformStrip` 逐 per-mille 投影，Iced Canvas 与 Bevy `sparkline_image` 画同一形状）、两个快捷开关、出口/模式/状态字母。**诚实偏差**：两端都不开第二个 OS 窗口——Iced 把宿主主窗口变成 HUD（真实 `move_to`/置顶，经桌面端口），Bevy 是窗口内浮层且不消费屏幕坐标（无拖拽、无独立置顶），`MiniHudWindowPort` 是未来真实悬浮窗的接缝 |
| `DUAL-15-04` | `parity-ready` | `crates/infiltrator-application/src/mini_hud_application.rs::place_snaps_clamps_persists_and_reports_the_host_outcome` | `crates/infiltrator-iced/src/mini_hud_store.rs::a_persisted_placement_reaches_the_iced_window_handle` | `crates/infiltrator-bevy-ui/tests/headless/mini_hud_tests.rs::the_pin_request_persists_through_the_shared_settings_command` | 桌面宿主实现 `DesktopMiniHudWindow`（`crates/infiltrator-desktop/src/mini_hud_window.rs::a_bound_handle_receives_the_placement_and_the_visibility_request`）：拖拽 → 共享 clamp/贴边吸附 → 落盘 → 真实窗口移动/置顶；未注册句柄的宿主（Bevy/无头）如实 `MiniHudHostOutcome::Unsupported`（`crates/infiltrator-desktop/src/mini_hud_window.rs::an_unbound_or_stale_handle_reports_typed_unsupported`） |
| `DUAL-15-05` | `parity-ready` | `crates/infiltrator-contract/src/command_catalogue.rs::catalogue_covers_every_shared_page_and_action_once` | `crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs::the_palette_lists_the_shared_catalogue_and_wraps_like_bevy` | `crates/infiltrator-bevy-ui/tests/headless/command_palette_tests.rs::test_palette_keyboard_navigation_typing_and_close` | Iced 保留本地编辑器路由，Bevy 无编辑器页；目录只收录双端可执行指令 |
| `DUAL-15-06` | `parity-ready` | `crates/infiltrator-application/src/shortcut_application.rs::a_stored_custom_chord_blocks_later_captures` | `crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs::the_keyboard_chord_dispatches_through_the_shared_registry` | `crates/infiltrator-bevy-ui/tests/headless/shortcut_tests.rs::capture_rebinds_through_the_shared_settings_command` | 无 |
| `DUAL-15-07` | `planned` | — | — | — | 手势引擎仅存于 widget 层纯实现（`infiltrator-bevy-widgets/src/gesture.rs`），无共享契约、无宿主接线 |
| `DUAL-15-08` | `parity-ready` | `crates/infiltrator-contract/src/cadence.rs::every_host_fact_maps_to_one_cadence` | `crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs::the_shell_tracks_window_focus_for_the_shared_cadence` | `crates/infiltrator-bevy-ui/tests/headless/cadence_tests.rs::focus_and_occlusion_events_reselect_the_winit_cadence` | 共享策略 60 FPS/2 FPS/挂起；Iced 只拿到焦点事实（无法区分最小化，绝不伪造挂起），Bevy 由焦点 + 遮挡事件驱动 `WinitSettings` |
| `DUAL-15-09` | `parity-ready` | `crates/infiltrator-contract/src/theme.rs::system_preference_resolves_to_the_os_appearance` | `crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs::the_shell_follows_the_os_appearance_while_the_preference_is_system` | `crates/infiltrator-bevy-ui/tests/headless/theme_skin_tests.rs::the_shell_follows_the_os_appearance_while_preference_is_system` | 无 |
| `DUAL-15-10` | `planned` | — | — | — | 无共享语义契约；Iced 全仓零 AccessKit 接入，Bevy 仅部分语义节点，无覆盖矩阵 |
| `DUAL-15-11` | `planned` | — | — | — | 全仓无 IME 组合事件/候选框定位处理，两端均未接线 |
| `DUAL-15-12` | `parity-ready` | `crates/infiltrator-contract/src/toast.rs::identical_toasts_inside_the_window_are_coalesced` | `crates/infiltrator-iced/tests/gui/business_flow/settings_lifecycle.rs::toast_lifecycle_redacts_secrets_and_survives_stale_removal` | `crates/infiltrator-bevy-ui/tests/headless/toast_overlay_tests.rs::a_sensitive_toast_is_redacted_before_it_renders` | 无 |
| `DUAL-15-13` | `planned` | — | — | — | Iced 启动窗口仍用默认装饰且无拖拽区；Bevy 无无边框拖拽接线；`display_adapter` 材料契约零引用 |
| `DUAL-15-14` | `shared-ready` | `crates/infiltrator-contract/src/design_tokens.rs::every_skin_has_a_distinct_core_palette` | `crates/infiltrator-iced/tests/gui/view_theme_tests.rs::theme_hairline_consumes_the_shared_contract_metric` | `crates/infiltrator-bevy-ui/tests/headless/design_token_tests.rs::the_widget_palette_mirrors_the_shared_design_tokens` | 声明范围＝四皮肤核心调色板 12 通道 + 间距/圆角阶梯 + 发丝线：Iced 全部按契约常量构造（发丝线已扫掉页面级 `width: 1.0` 字面量），Bevy widget 层按数值镜像，守卫 `multimodal-shell-guard.py::check_design_token_mirrors` 逐 token 比对且 `check_no_raw_hairlines` 禁止 Iced 回退到裸 1.0；**缩小的范围**：overlay 气泡、hover/pressed 洗色、icon tile 染色、阴影与字号阶梯仍是各表面本地装饰，02/07/10/11/13 未闭环，1:1 质感对齐未达成 |
| `DUAL-15-15` | `shared-ready` | `crates/infiltrator-contract/src/cadence.rs::the_product_rates_are_sixty_and_two_fps` | `crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs::the_placement_update_drains_the_host_window_requests` | `crates/infiltrator-bevy-ui/tests/headless/design_token_tests.rs::the_widget_ladders_mirror_the_shared_contract_numbers` | 本矩阵即该项产物：守卫现在逐行校验「状态与主计划账目一致、`parity-ready` 行必须有共享+Iced+Bevy 三侧 evidence、每个引用文件与测试名真实存在、`planned` 行不得声称测试」；02/07/10/11/13 无测试面，矩阵未 100% |

> 矩阵维护口径：状态或 evidence 变化必须同时更新本表、`docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md`
> 的组 15 逐项账目与 `scripts/quality/multimodal-shell-guard.py`；三处不一致即守卫失败。
