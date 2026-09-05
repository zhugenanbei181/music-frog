use super::*;

#[test]
fn test_group_icon_mapping() {
    assert_eq!(group_icon("URLTest"), Icon::Zap);
    assert_eq!(group_icon("url-test"), Icon::Zap);
    assert_eq!(group_icon("UrlTest"), Icon::Zap);
    assert_eq!(group_icon("Fallback"), Icon::Shield);
    assert_eq!(group_icon("fallback"), Icon::Shield);
    assert_eq!(group_icon("LoadBalance"), Icon::ListChecks);
    assert_eq!(group_icon("load-balance"), Icon::ListChecks);
    assert_eq!(group_icon("Load-Balance"), Icon::ListChecks);
    assert_eq!(group_icon("Selector"), Icon::Globe);
    assert_eq!(group_icon("selector"), Icon::Globe);
    assert_eq!(group_icon("something_else"), Icon::Globe);
}

#[test]
fn test_format_protocol_chip() {
    assert_eq!(format_protocol_chip("Shadowsocks"), "Shadowsocks");
    assert_eq!(format_protocol_chip("ss"), "Shadowsocks");
    assert_eq!(format_protocol_chip("Vless"), "Vless");
    assert_eq!(format_protocol_chip("vless"), "Vless");
    assert_eq!(format_protocol_chip("vmess"), "VMess");
    assert_eq!(format_protocol_chip("VMess"), "VMess");
    assert_eq!(format_protocol_chip("Trojan"), "Trojan");
    assert_eq!(format_protocol_chip("trojan"), "Trojan");
    assert_eq!(format_protocol_chip("Hysteria2"), "Hysteria2");
    assert_eq!(format_protocol_chip("hy2"), "Hysteria2");
    assert_eq!(format_protocol_chip("wireguard"), "WireGuard");
    assert_eq!(format_protocol_chip("tuic"), "Tuic");
    assert_eq!(format_protocol_chip("http"), "HTTP");
    assert_eq!(format_protocol_chip("socks5"), "SOCKS5");
    assert_eq!(format_protocol_chip("snell"), "Snell");
    assert_eq!(format_protocol_chip("direct"), "Direct");
    assert_eq!(format_protocol_chip("reject"), "Reject");
    assert_eq!(format_protocol_chip("custom-proto"), "custom-proto");
    assert_eq!(format_protocol_chip(""), "Proxy");
}

#[test]
fn test_sort_keys_correspondence() {
    assert_eq!(SORT_KEYS.len(), SORT_LABEL_KEYS.len());
    assert_eq!(SORT_KEYS[0], "delay_asc");
    assert_eq!(SORT_KEYS[1], "delay_desc");
    assert_eq!(SORT_KEYS[2], "name_asc");
    assert_eq!(SORT_KEYS[3], "name_desc");
}
