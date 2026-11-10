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
        "relative_time_just_now" => "刚刚".into(),
        "relative_time_minutes_ago" => "{count} 分钟前".into(),
        "relative_time_hours_ago" => "{count} 小时前".into(),
        "relative_time_days_ago" => "{count} 天前".into(),
        "relative_time_months_ago" => "{count} 个月前".into(),
        "relative_time_years_ago" => "{count} 年前".into(),
        "relative_time_compact_just_now" => "刚刚".into(),
        "relative_time_compact_minutes_ago" => "{count} 分".into(),
        "relative_time_compact_hours_ago" => "{count} 小时".into(),
        "relative_time_compact_days_ago" => "{count} 天".into(),
        "relative_time_compact_months_ago" => "{count} 月".into(),
        "relative_time_compact_years_ago" => "{count} 年".into(),
        "relative_time_detailed_prefix" => "更新于 {time}".into(),
        "profile_source_local" => "本地".into(),
        "profile_source_subscription" => "远程".into(),
        "profile_source_imported" => "导入".into(),
        "profile_source_synchronized" => "同步".into(),
        "profile_source_unobserved" => "来源未知".into(),
        _ => key.to_string().into(),
    }
}
