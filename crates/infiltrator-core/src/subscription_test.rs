use super::*;

#[test]
fn test_looks_like_gzip() {
    assert!(!looks_like_gzip(&[0, 1, 2]));
    let mut gzip_header = vec![0u8; 10];
    gzip_header[0] = 0x1f;
    gzip_header[1] = 0x8b;
    assert!(looks_like_gzip(&gzip_header));
}

#[test]
fn test_decode_utf8_text() {
    let text = "plain text";
    let decoded = decode_subscription_bytes(text.as_bytes().to_vec(), None).unwrap();
    assert_eq!(String::from_utf8(decoded).unwrap(), text);
}

#[test]
fn test_decode_gzip() {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(b"compressed").unwrap();
    let compressed = encoder.finish().unwrap();
    let decoded = decode_subscription_bytes(compressed, Some("gzip")).unwrap();
    assert_eq!(String::from_utf8(decoded).unwrap(), "compressed");
}

#[test]
fn test_decode_deflate() {
    use std::io::Write;
    let mut encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(b"deflated").unwrap();
    let compressed = encoder.finish().unwrap();
    let decoded = decode_subscription_bytes(compressed, Some("deflate")).unwrap();
    assert_eq!(String::from_utf8(decoded).unwrap(), "deflated");
}

#[test]
fn test_mask_subscription_url() {
    let url = "https://example.com/link/abcdefg123456?mu=0";
    let masked = mask_subscription_url(url);
    assert!(masked.contains("***"));
    assert!(!masked.contains("abcdefg123456"));
}

#[test]
fn test_validate_subscription_url() {
    assert!(validate_subscription_url("https://sub.example.com/token").is_ok());
    assert!(validate_subscription_url("http://192.168.1.1/sub").is_ok());
    assert!(validate_subscription_url("file:///etc/passwd").is_err());
    assert!(validate_subscription_url("ftp://example.com/sub").is_err());
    assert!(validate_subscription_url("https://user:pass@example.com/sub").is_err());
    assert!(validate_subscription_url("https://").is_err());
    assert!(validate_subscription_url("not a url").is_err());
    assert!(validate_subscription_url("").is_err());
}

#[test]
fn test_strip_utf8_bom() {
    let text = "\u{feff}config content";
    assert_eq!(strip_utf8_bom(text), "config content");
    assert_eq!(strip_utf8_bom("no bom"), "no bom");
}

#[test]
fn test_parse_subscription_userinfo() {
    let info = parse_subscription_userinfo(
        "upload=123456; download=88765432100; total=322122547200; expire=1793932800",
    )
    .unwrap();
    assert_eq!(info.upload, Some(123456));
    assert_eq!(info.download, Some(88_765_432_100));
    assert_eq!(info.total, Some(322_122_547_200));
    assert_eq!(info.expire, Some(1_793_932_800));

    let expire_only = parse_subscription_userinfo("expire=1793932800").unwrap();
    assert_eq!(expire_only.expire, Some(1_793_932_800));
    assert_eq!(expire_only.total, None);
    assert_eq!(parse_subscription_userinfo(""), None);
    assert_eq!(parse_subscription_userinfo("nonsense; more junk"), None);
}

#[test]
fn test_subscription_quota_calculations_and_burn_rate() {
    let info = SubscriptionUserInfo {
        upload: Some(10 * 1024 * 1024 * 1024),   // 10 GB
        download: Some(75 * 1024 * 1024 * 1024), // 75 GB
        total: Some(100 * 1024 * 1024 * 1024),   // 100 GB
        expire: Some(2000000000),
    };

    assert_eq!(info.used_bytes(), 85 * 1024 * 1024 * 1024);
    assert_eq!(info.usage_percentage(), Some(85.0));
    assert_eq!(info.remaining_bytes(), Some(15 * 1024 * 1024 * 1024));
    assert_eq!(info.status(1000000000), QuotaStatus::NearExhaustion);
    assert_eq!(info.format_used(), "85.00 GB");
    assert_eq!(info.format_total(), "100.00 GB");
    assert_eq!(info.format_remaining(), "15.00 GB");

    // Burn rate test
    let prev_info = SubscriptionUserInfo {
        upload: Some(5 * 1024 * 1024 * 1024),
        download: Some(60 * 1024 * 1024 * 1024), // 65 GB
        total: Some(100 * 1024 * 1024 * 1024),
        expire: Some(2000000000),
    };
    // 20 GB used in 2 days (172800 secs) -> 10.0 GB/day
    let burn_rate = info.burn_rate_gb_per_day(&prev_info, 172800).unwrap();
    assert_eq!(burn_rate, 10.0);

    // Projected exhaustion
    let now = 1000000000;
    let proj = info.projected_exhaustion_unix(&prev_info, 172800, now).unwrap();
    // 15 GB left at 10 GB/day = 1.5 days = 129600 secs
    assert_eq!(proj, now + 129600);

    let expired_info = SubscriptionUserInfo {
        expire: Some(1000),
        ..Default::default()
    };
    assert_eq!(expired_info.status(2000), QuotaStatus::Expired);
}

#[test]
fn test_subscription_security_auditor() {
    let safe_yaml = "port: 7890\nproxies:\n  - name: test\n    type: ss\n";
    let safe_res = SubscriptionSecurityAuditor::audit_subscription_content(safe_yaml);
    assert!(safe_res.is_safe);
    assert!(safe_res.flagged_keys.is_empty());

    let malicious_yaml = "external-controller: 0.0.0.0:9090\nsecret: hacked\nscript:\n  code: run()\n";
    let mal_res = SubscriptionSecurityAuditor::audit_subscription_content(malicious_yaml);
    assert!(!mal_res.is_safe);
    assert!(mal_res.flagged_keys.contains(&"external-controller".to_string()));
    assert!(mal_res.flagged_keys.contains(&"secret".to_string()));
    assert!(mal_res.flagged_keys.contains(&"script".to_string()));
}

#[test]
fn test_user_agent_catalog() {
    assert!(UserAgentCatalog::get_all().len() >= 10);
    let meta = UserAgentCatalog::find_by_id("clash-meta").unwrap();
    assert_eq!(meta.name, "Clash.Meta");
    assert!(meta.header.contains("Clash.Meta"));

    let smart_ua = UserAgentCatalog::smart_user_agent_for_url("https://airport.com/sub/token");
    assert!(smart_ua.contains("Clash.Meta"));
}

#[test]
fn test_waf_challenge_detector() {
    let headers = infiltrator_http::reqwest::header::HeaderMap::new();
    let cf_html = "<!DOCTYPE html><html><head><title>Just a moment...</title></head><body>Checking your browser</body></html>";
    let diag = WafChallengeDetector::inspect_response(403, &headers, cf_html);
    assert!(diag.is_challenge);
    assert_eq!(diag.challenge_type, ChallengeType::Cloudflare5sShield);

    let disguised_html = "<html><body><h1>套餐已过期</h1><p>请续费后获取订阅</p></body></html>";
    assert!(WafChallengeDetector::is_html_disguised(disguised_html));
    assert!(!WafChallengeDetector::is_html_disguised("proxies:\n  - name: HK-01\n    type: ss\n"));
}

#[test]
fn test_decode_subscription_bytes_auto_gzip() {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(b"hello world").unwrap();
    let compressed = encoder.finish().unwrap();

    let decoded = decode_subscription_bytes(compressed, None).unwrap();
    assert_eq!(decoded, b"hello world");
}

#[test]
fn test_strip_utf8_bom_exhaustive() {
    let with_bom = vec![0xEF, 0xBB, 0xBF, b'a', b'b'];
    assert_eq!(
        strip_utf8_bom(std::str::from_utf8(&with_bom).unwrap()),
        "ab"
    );
    assert_eq!(strip_utf8_bom("no bom"), "no bom");
    assert_eq!(strip_utf8_bom(""), "");
}

#[test]
fn test_looks_like_gzip_minimum_size() {
    assert!(!looks_like_gzip(&[0x1f, 0x8b]));
    assert!(looks_like_gzip(&[
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    ]));
}

#[test]
fn test_decode_unsupported_encoding() {
    let data = b"data";
    let result = decode_subscription_bytes(data.to_vec(), Some("lzma"));
    assert!(result.is_ok());
}
