use crate::locales::*;

#[test]
fn test_resolve_language_code() {
    assert_eq!(resolve_language_code("en"), "en-US");
    assert_eq!(resolve_language_code("EN"), "en-US");
    assert_eq!(resolve_language_code("zh-CN"), "zh-CN");
    assert_eq!(resolve_language_code("fr-FR"), "fr-FR");
}

#[test]
fn test_normalize_locale() {
    assert_eq!(normalize_locale("zh-TW"), "zh-CN");
    assert_eq!(normalize_locale("ZH-CN"), "zh-CN");
    assert_eq!(normalize_locale("en-GB"), "en-US");
    assert_eq!(normalize_locale("ja-JP"), "en-US");
}

#[test]
fn test_translations_zh() {
    let lang = Lang("zh-CN");
    assert_eq!(lang.tr("enabled"), "已开启");
    assert_eq!(lang.tr("disabled"), "已关闭");
    assert_eq!(lang.tr("nonexistent_key"), "nonexistent_key");
}

#[test]
fn test_translations_en() {
    let lang = Lang("en-US");
    assert_eq!(lang.tr("enabled"), "Enabled");
    assert_eq!(lang.tr("disabled"), "Disabled");
    assert_eq!(lang.tr("nonexistent_key"), "nonexistent_key");
}

#[test]
fn test_lang_alias_en() {
    let lang = Lang("en");
    assert_eq!(lang.tr("settings"), "Settings");
}

#[test]
fn test_shared_product_catalog_localizes_slint_copy_and_stable_runtime_keys() {
    let english = Lang("en-US");
    let chinese = Lang("zh-CN");
    assert_eq!(english.tr("VPN 已连接"), "VPN connected");
    assert_eq!(chinese.tr("VPN 已连接"), "VPN 已连接");
    assert_eq!(english.tr("public_ip_ready"), "Egress IP is available");
    assert_eq!(chinese.tr("public_ip_ready"), "出口 IP 已可用");
    // A historical PO entry without a blank separator must still be loaded.
    assert_eq!(english.tr("上传  "), "Upload  ");
}

#[test]
fn test_locales_table_key_parity() {
    let keys = [
        "app_title",
        "nav_overview",
        "nav_profiles",
        "nav_proxies",
        "nav_runtime",
        "nav_rules",
        "nav_dns",
        "nav_sync",
        "nav_settings",
        "sync_title",
        "settings_admin_open",
        "settings_uac_unsupported",
        "tray_factory_reset",
        "notify_rebuild_failed",
        "traffic_expires",
        "rules_title",
        "rules_etag_support_label",
        "rules_etag_support_enabled",
        "rules_etag_support_disabled",
        "rules_etag_support_not_declared",
    ];
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    for key in keys {
        assert_ne!(zh.tr(key), key, "zh key {key} must have translation");
        assert_ne!(en.tr(key), key, "en key {key} must have translation");
    }
}
