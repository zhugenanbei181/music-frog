//! Shared-copy catalog for the Chinese product locale.
use std::borrow::Cow;

pub(super) fn translate_shared_copy_zh(key: &str) -> Cow<'static, str> {
    match key {
        "brand_name" => "MusicFrog".into(),
        "app_display_name" => "MusicFrog Infiltrator".into(),
        "configuration_saving" => "保存配置".into(),
        "configuration_saved" => "已保存".into(),
        "configuration_failed" => "保存失败".into(),
        "performance_snapshot" => "性能快照".into(),
        "performance_navigate_to_first_paint" => "导航至首帧：{value}".into(),
        "common_close" => "关闭".into(),
        "common_retry" => "重试".into(),
        "dismiss_probe_batch" => "关闭".into(),
        "layout_density" => "布局密度".into(),
        "a11y_value_prefix" => "{label}：{value}".into(),
        _ => key.to_string().into(),
    }
}
