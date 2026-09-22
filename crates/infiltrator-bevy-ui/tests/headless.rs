#[path = "headless/support.rs"]
mod support;

#[path = "headless/shell_tests.rs"]
mod shell_tests;

#[path = "headless/overview_tests.rs"]
mod overview_tests;

#[path = "headless/controller_tests.rs"]
mod controller_tests;

#[path = "headless/pages_matrix_tests.rs"]
mod pages_matrix_tests;

#[path = "headless/pages_matrix_a_tests.rs"]
mod pages_matrix_a_tests;

#[path = "headless/pages_matrix_b_tests.rs"]
mod pages_matrix_b_tests;

#[path = "headless/responsive_ui_tests.rs"]
mod responsive_ui_tests;

#[path = "headless/command_palette_tests.rs"]
mod command_palette_tests;

#[path = "headless/mini_hud_tests.rs"]
mod mini_hud_tests;

#[path = "headless/surface_tests.rs"]
mod surface_tests;

#[path = "headless/theme_skin_tests.rs"]
mod theme_skin_tests;

#[path = "headless/shortcut_tests.rs"]
mod shortcut_tests;

#[path = "headless/toast_overlay_tests.rs"]
mod toast_overlay_tests;

#[path = "headless/design_token_tests.rs"]
mod design_token_tests;

#[path = "headless/window_chrome_tests.rs"]
mod window_chrome_tests;

#[path = "headless/a11y_semantics_tests.rs"]
mod a11y_semantics_tests;

#[path = "headless/tray_status_tests.rs"]
mod tray_status_tests;

#[path = "headless/cadence_tests.rs"]
mod cadence_tests;

// DUAL-05: protocol-ecosystem studio (custom node URI codec) dual-surface tests.
#[path = "headless/protocol_codec_tests.rs"]
mod protocol_codec_tests;
