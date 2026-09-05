use super::*;

#[test]
fn test_format_bytes_scale() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1024), "1.00 KB");
    assert_eq!(format_bytes(1048576), "1.00 MB");
    assert_eq!(format_bytes(1073741824), "1.00 GB");
    assert_eq!(format_bytes(1099511627776), "1.00 TB");
}

#[test]
fn test_format_datetime_fallback() {
    assert_eq!(format_datetime(None, "Never"), "Never");
    let fixed = DateTime::from_timestamp(1700000000, 0).unwrap();
    let formatted = format_datetime(Some(fixed), "Never");
    assert!(!formatted.is_empty());
    assert_ne!(formatted, "Never");
}

#[test]
fn test_traffic_row_none_without_info() {
    let p = ProfileInfo {
        name: "test".to_string(),
        path: "/tmp/test.yaml".to_string(),
        ..Default::default()
    };
    assert!(traffic_row(&p, &Lang("zh-CN")).is_none());
    assert!(traffic_row(&p, &Lang("en-US")).is_none());
}

#[test]
fn test_traffic_row_with_quota_and_expire() {
    let mut p = ProfileInfo {
        name: "sub".to_string(),
        path: "/tmp/sub.yaml".to_string(),
        active: true,
        ..Default::default()
    };
    p.subscription_url = Some("https://example.com/sub".to_string());
    p.traffic_upload = Some(1024 * 1024 * 100);
    p.traffic_download = Some(1024 * 1024 * 900);
    p.traffic_total = Some(1024 * 1024 * 1000);
    p.expire_at = Some(1900000000);

    assert!(traffic_row(&p, &Lang("zh-CN")).is_some());
    assert!(traffic_row(&p, &Lang("en-US")).is_some());
}

#[test]
fn test_traffic_row_threshold_tiers() {
    let mut p = ProfileInfo {
        name: "sub".to_string(),
        path: "/tmp/sub.yaml".to_string(),
        active: true,
        ..Default::default()
    };
    p.subscription_url = Some("https://example.com/sub".to_string());
    p.traffic_total = Some(1000);

    p.traffic_download = Some(300);
    assert!(traffic_row(&p, &Lang("zh-CN")).is_some());

    p.traffic_download = Some(650);
    assert!(traffic_row(&p, &Lang("zh-CN")).is_some());

    p.traffic_download = Some(850);
    assert!(traffic_row(&p, &Lang("zh-CN")).is_some());

    p.traffic_download = Some(950);
    assert!(traffic_row(&p, &Lang("zh-CN")).is_some());

    p.traffic_download = Some(1050);
    assert!(traffic_row(&p, &Lang("zh-CN")).is_some());
}

#[test]
fn test_ua_preset_chip_widget() {
    let _chip1: Element<'_, Message> = ua_preset_chip("Clash.Meta", "Clash.Meta");
    let _chip2: Element<'_, Message> = ua_preset_chip("ClashVerge", "Clash.Meta");
    let _chip3: Element<'_, Message> = ua_preset_chip("Shadowrocket", "");
}
