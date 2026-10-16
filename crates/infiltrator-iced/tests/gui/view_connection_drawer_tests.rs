//! DUAL-13-05: kernel-provided target ASN / geolocation rendering tests.

use super::*;

#[test]
fn test_kernel_asn_label_separates_not_evaluated_from_no_result() {
    let zh = Lang("zh-CN");
    let en = Lang("en");

    // The kernel sent `""`: no IP-ASN rule ran for this connection.
    assert_eq!(
        kernel_asn_label("", &|key| zh.tr(key).into_owned()),
        "内核未对本次连接求值（需 GEOIP/IP-ASN 规则）"
    );
    // It sent `" "` (number + space + organization): it evaluated and had no
    // record for the IP.
    assert_eq!(
        kernel_asn_label(" ", &|key| zh.tr(key).into_owned()),
        "内核已求值 · 无该 IP 的记录"
    );
    // A real kernel value is rendered verbatim: the client adds no `AS` prefix
    // and never substitutes an ASN of its own.
    assert_eq!(
        kernel_asn_label("15169 Google LLC", &|key| zh.tr(key).into_owned()),
        "15169 Google LLC"
    );
    assert_eq!(
        kernel_asn_label("15169 Google LLC", &|key| en.tr(key).into_owned()),
        "15169 Google LLC"
    );
}

#[test]
fn test_kernel_geo_label_keeps_the_kernels_three_states() {
    let zh = Lang("zh-CN");

    // `null`: the kernel never queried a GEOIP rule.
    assert_eq!(
        kernel_geo_label(None, &|key| zh.tr(key).into_owned()),
        "内核未对本次连接求值（需 GEOIP/IP-ASN 规则）"
    );
    // `[]`: queried with no record.
    assert_eq!(
        kernel_geo_label(Some(&[]), &|key| zh.tr(key).into_owned()),
        "内核已求值 · 无该 IP 的记录"
    );
    // Real codes: rendered in kernel order with no client-side mapping.
    let codes = vec!["us".to_owned(), "cloudflare".to_owned()];
    assert_eq!(
        kernel_geo_label(Some(&codes), &|key| zh.tr(key).into_owned()),
        "us, cloudflare"
    );
}
