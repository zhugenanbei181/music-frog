//! Shared network workbench copy.
use std::borrow::Cow;

pub(super) fn translate_network_zh(key: &str) -> Cow<'static, str> {
    match key {
        "dns_field_enable" => "enable（启用 DNS 服务）".into(),
        "dns_field_ipv6" => "ipv6（IPv6 解析）".into(),
        "dns_field_cache" => "cache（DNS 内存缓存）".into(),
        "dns_field_use_hosts" => "use_hosts（遵循系统 Hosts）".into(),
        "dns_field_use_system_hosts" => "use_system_hosts（系统 Hosts）".into(),
        "dns_field_respect_rules" => "respect_rules（分流规则优先）".into(),
        "dns_field_enhanced_mode" => "enhanced_mode（域名映射模式）".into(),
        "dns_field_filter_mode" => "fake_ip_filter_mode（Fake-IP 过滤模式）".into(),
        "dns_field_bootstrap" => "default_nameserver（bootstrap，仅纯 IP）".into(),
        "dns_field_nameserver" => "nameserver（DoH / DoT / DoQ / UDP）".into(),
        "dns_field_fallback" => "fallback（回退解析服务器）".into(),
        "dns_field_geoip" => "fallback_filter.geoip（GEOIP 回退触发）".into(),
        "dns_field_geoip_code" => "fallback_filter.geoip_code（ISO 国家代码）".into(),
        "dns_field_trigger" => "fallback_filter.ipcidr（GEOIP 触发网段）".into(),
        "dns_field_fake_range" => "fake_ip_range".into(),
        "dns_field_fake_filter" => "fake_ip_filter".into(),
        "dns_field_proxy_nameserver" => "proxy_server_nameserver".into(),
        "dns_field_direct_nameserver" => "direct_nameserver".into(),
        "dns_trigger_count" => "fallback_filter.ipcidr 触发网段 {count} 条".into(),
        "dns_edit_invalid" => "本地校验未通过: {reason}".into(),
        "dns_edit_dirty" => "有未应用的修改，点击应用提交共享补丁".into(),
        "dns_edit_queued" => "已提交共享 DNS 工作台补丁，等待实际结果".into(),
        "dns_edit_unchanged" => "表单与当前配置一致".into(),
        "dns_edit_host_unavailable" => "命令服务未装配，未提交 DNS 补丁".into(),
        "network_status_detail" => "{state} · {reason}".into(),
        "network_vpn_details" => "foreground={foreground} · MTU={mtu} · routes={routes} · IPv6={ipv6}".into(),
        "network_privileged_details" => "operations={operations} · injected={injected} · cleanup={cleanup} · rollback={rollback}".into(),
        "network_pac_running" => "运行中 · {url} · {bytes} 字节".into(),
        "network_pac_disabled" => "未启用".into(),
        _ => Cow::Owned(key.to_owned()),
    }
}
