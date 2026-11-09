//! Shared-copy catalog for the Chinese product locale.
use infiltrator_contract::surface::SurfaceKind;
use std::borrow::Cow;

pub(super) fn translate_shared_copy_zh(key: &str) -> Cow<'static, str> {
    match key {
        "brand_name" => SurfaceKind::IcedDesktop.product_name().into(),
        "app_display_name" => SurfaceKind::IcedDesktop.product_name().into(),
        "about_brand_note" => "图标中的青蛙乐手是 MusicFrog 内部标识；产品名只保留本名。".into(),
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
        "capability_not_registered" => "当前宿主未登记此能力".into(),
        _ => key.to_string().into(),
    }
}
