//! Demo data fixtures: traffic history, mihomo-style logs, connection
//! snapshots, rules, profiles and the JSON/YAML editor cache contents.

use infiltrator_core::rules::RuleEntry;
use mihomo_api::types::{Connection, ConnectionMetadata, ConnectionSnapshot};
use mihomo_config::profile::Profile;
use std::collections::VecDeque;
use std::path::PathBuf;

/// 60 samples (the app's own history cap and the chart's `max_points`) of a
/// believable wave: up 0.5–4 MB/s, down 1–12 MB/s.
pub(super) fn demo_traffic_history() -> VecDeque<(u64, u64)> {
    const MB: f64 = 1024.0 * 1024.0;
    (0..60u32)
        .map(|i| {
            let up_wave = 0.5 + (f64::from(i) * 0.35).sin().mul_add(0.5, 0.5) * 3.5;
            let down_wave = (f64::from(i) * 0.22 + 1.3)
                .sin()
                .mul_add(0.5, 0.5)
                .mul_add(11.0, 1.0);
            ((up_wave * MB) as u64, (down_wave * MB) as u64)
        })
        .collect()
}

/// ~40 mixed mihomo-style log lines (info/warn/error, Chinese included).
pub(super) fn demo_logs() -> VecDeque<String> {
    let lines: &[&str] = &[
        "INFO[0001] Start initial configuration in progress",
        "INFO[0001] MMDB(geoip.metadb) 已加载，包含 320547 条记录",
        "INFO[0002] Level-1 负载均衡已启用",
        "INFO[0002] RESTful API listening at 127.0.0.1:9090",
        "INFO[0003] [TCP] 192.168.1.23:52118 --> www.google.com:443 match DomainSuffix(google.com) using 节点选择[香港 IEPL-01]",
        "INFO[0004] [UDP] 192.168.1.23:52119 --> 8.8.8.8:53 match Ip CIDR(8.8.8.8/32) using 全球直连[DIRECT]",
        "WARN[0006] [TCP] connect error (ZGO): dial tcp 104.21.0.0:36712: i/o timeout",
        "INFO[0007] [TCP] 192.168.1.23:52124 --> api.openai.com:443 match DomainSuffix(openai.com) using AI 服务[美国 CN2]",
        "INFO[0008] [TCP] 192.168.1.23:52130 --> www.youtube.com:443 match DomainKeyword(youtube) using 节点选择[香港 IEPL-01]",
        "ERROR[0009] DNS 解析失败：resolver default: lookup doh.privatedns.example: server misbehaving",
        "INFO[0010] [TCP] 192.168.1.23:52131 --> github.com:443 match DomainSuffix(github.com) using 节点选择[DMIT]",
        "INFO[0011] external controller 已就绪",
        "WARN[0013] provider 机场订阅 的更新间隔小于推荐值 (24h)",
        "INFO[0014] [TCP] 192.168.1.23:52140 --> cdn.jsdelivr.net:443 match DomainSuffix(jsdelivr.net) using 节点选择[香港 IEPL-02]",
        "INFO[0015] [TCP] 192.168.1.23:52144 --> mail.qq.com:443 match DomainSuffix(qq.com) using 全球直连[DIRECT]",
        "INFO[0016] Sniffer 已启用：对 443 端口执行 TLS 嗅探",
        "INFO[0017] [UDP] 192.168.1.23:52150 --> 1.1.1.1:53 match RuleSet(cn-cidr) using 全球直连[DIRECT]",
        "ERROR[0019] 延迟测试失败 (ZGO): context deadline exceeded",
        "INFO[0020] [TCP] 192.168.1.23:52155 --> store.steampowered.com:443 match DomainSuffix(steamcontent.com) using 游戏平台[日本 NTT]",
        "INFO[0021] [TCP] 192.168.1.23:52158 --> chat.openai.com:443 match DomainSuffix(openai.com) using AI 服务[美国 CN2]",
        "WARN[0023] TUN 未启用，系统代理模式运行中",
        "INFO[0024] [TCP] 192.168.1.23:52160 --> www.bilibili.com:443 match GeoIP(CN) using 全球直连[DIRECT]",
        "INFO[0025] Fake-IP 缓存已持久化 (198.18.0.1/16)",
        "INFO[0026] [TCP] 192.168.1.23:52166 --> api.telegram.org:443 match DomainSuffix(telegram.org) using 节点选择[新加坡 BGP]",
        "ERROR[0028] WebDAV 同步失败：401 Unauthorized (请检查账号密码)",
        "INFO[0029] [TCP] 192.168.1.23:52170 --> twitch.tv:443 match DomainSuffix(twitch.tv) using 节点选择[DMIT]",
        "INFO[0030] 规则集 google 更新完成 (785 条)",
        "INFO[0031] [UDP] 192.168.1.23:52175 --> 223.5.5.5:443 match RuleSet(cn-cidr) using 全球直连[DIRECT]",
        "INFO[0032] [TCP] 192.168.1.23:52180 --> www.icloud.com:443 match RuleSet(icloud) using 全球直连[DIRECT]",
        "WARN[0034] 节点 香港 IEPL-02 连续 3 次延迟高于 200ms",
        "INFO[0035] [TCP] 192.168.1.23:52188 --> twitter.com:443 match DomainSuffix(twitter.com) using 节点选择[DMIT]",
        "INFO[0036] 内存占用：92.00 MB (上限 0)",
        "INFO[0037] [TCP] 192.168.1.23:52192 --> xiaomi.com:443 match DomainSuffix(xiaomi.com) using 全球直连[DIRECT]",
        "INFO[0038] 订阅更新成功：机场订阅 (24h 后自动更新)",
        "INFO[0039] [TCP] 192.168.1.23:52196 --> edge.microsoft.com:443 match DomainSuffix(microsoft.com) using 节点选择[香港 IEPL-01]",
        "ERROR[0041] 连接中断 (游戏平台[日本 NTT]): connection reset by peer",
        "INFO[0042] 自动重连成功：api.openai.com:443",
        "INFO[0043] [TCP] 192.168.1.23:52200 --> v2ex.com:443 match Match(漏网之鱼) using 节点选择[香港 IEPL-01]",
        "INFO[0044] 当前出口：香港 IEPL-01 (148ms)",
        "INFO[0045] 心跳正常，运行时间 02:41:17",
    ];
    lines.iter().map(|s| s.to_string()).collect()
}

/// 10 mixed connection rows (hosts / ports / rules / chains / traffic).
pub(super) fn demo_connections() -> ConnectionSnapshot {
    // (host, port, rule type, rule payload, chain group, exit node, down, up)
    type DemoRow = (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        u64,
        u64,
    );
    let rows: [DemoRow; 10] = [
        (
            "www.google.com",
            "443",
            "DomainSuffix",
            "google.com",
            "节点选择",
            "香港 IEPL-01",
            48_234_112,
            2_097_152,
        ),
        (
            "api.openai.com",
            "443",
            "DomainSuffix",
            "openai.com",
            "AI 服务",
            "美国 CN2",
            12_884_901,
            6_291_456,
        ),
        (
            "www.youtube.com",
            "443",
            "DomainKeyword",
            "youtube",
            "节点选择",
            "香港 IEPL-01",
            335_544_320,
            15_728_640,
        ),
        (
            "github.com",
            "443",
            "DomainSuffix",
            "github.com",
            "节点选择",
            "DMIT",
            22_020_096,
            4_194_304,
        ),
        (
            "cdn.jsdelivr.net",
            "443",
            "DomainSuffix",
            "jsdelivr.net",
            "节点选择",
            "香港 IEPL-02",
            8_388_608,
            1_572_864,
        ),
        (
            "mail.qq.com",
            "443",
            "DomainSuffix",
            "qq.com",
            "全球直连",
            "DIRECT",
            6_291_456,
            2_621_440,
        ),
        (
            "www.bilibili.com",
            "443",
            "GeoIP",
            "CN",
            "全球直连",
            "DIRECT",
            18_874_368,
            3_355_443,
        ),
        (
            "store.steampowered.com",
            "443",
            "DomainSuffix",
            "steamcontent.com",
            "游戏平台",
            "日本 NTT",
            96_468_992,
            8_388_608,
        ),
        (
            "chat.openai.com",
            "443",
            "DomainSuffix",
            "openai.com",
            "AI 服务",
            "新加坡 BGP",
            5_242_880,
            1_048_576,
        ),
        (
            "v2ex.com",
            "443",
            "Match",
            "漏网之鱼",
            "节点选择",
            "香港 IEPL-01",
            3_145_728,
            786_432,
        ),
    ];

    let connections = rows
        .iter()
        .enumerate()
        .map(
            |(i, (host, port, rule, payload, chain, node, download, upload))| Connection {
                id: format!("demo-conn-{i:03}"),
                metadata: ConnectionMetadata {
                    network: if i % 4 == 3 { "udp" } else { "tcp" }.to_string(),
                    connection_type: if i % 4 == 3 { "UDP" } else { "TLS" }.to_string(),
                    source_ip: "192.168.1.23".to_string(),
                    destination_ip: format!("203.0.113.{}", 10 + i),
                    source_port: format!("{}", 52_100 + i * 7),
                    destination_port: port.to_string(),
                    host: host.to_string(),
                    dns_mode: "fake-ip".to_string(),
                    process_path: if i % 3 == 0 {
                        "/usr/bin/chromium".to_string()
                    } else {
                        String::new()
                    },
                    special_proxy: String::new(),
                },
                upload: *upload,
                download: *download,
                start: format!(
                    "2026-08-29T15:{:02}:{:02}.000000+08:00",
                    20 + i,
                    (i * 7) % 60
                ),
                rule: rule.to_string(),
                rule_payload: payload.to_string(),
                chains: vec![node.to_string(), chain.to_string()],
            },
        )
        .collect();

    ConnectionSnapshot {
        download_total: 3_758_096_384,
        upload_total: 728_766_464,
        connections,
    }
}

/// 15 rules covering the DOMAIN* / IP-CIDR / GEOIP / RuleSet / MATCH families.
pub(super) fn demo_rules() -> Vec<RuleEntry> {
    let rules = [
        "DOMAIN-SUFFIX,google.com,节点选择",
        "DOMAIN-SUFFIX,openai.com,AI 服务",
        "DOMAIN-KEYWORD,youtube,节点选择",
        "DOMAIN-SUFFIX,telegram.org,节点选择",
        "DOMAIN-SUFFIX,github.com,节点选择",
        "DOMAIN-SUFFIX,qq.com,DIRECT",
        "DOMAIN-SUFFIX,bilibili.com,DIRECT",
        "DOMAIN-SUFFIX,xiaomi.com,DIRECT",
        "DOMAIN-SUFFIX,steamcontent.com,游戏平台",
        "DOMAIN-SUFFIX,microsoft.com,节点选择",
        "IP-CIDR,8.8.8.8/32,全球直连,no-resolve",
        "IP-CIDR,192.168.0.0/16,DIRECT,no-resolve",
        "IP-CIDR6,2620:0:2d0:200::7/32,REJECT,no-resolve",
        "GEOIP,CN,DIRECT",
        "MATCH,漏网之鱼",
    ];
    rules
        .into_iter()
        .map(|rule| RuleEntry {
            rule: rule.to_string(),
            enabled: true,
        })
        .collect()
}

/// 3 profiles: an active subscription, a local file and a standby subscription.
pub(super) fn demo_profiles() -> Vec<Profile> {
    use chrono::TimeZone;
    let updated = chrono::Utc
        .with_ymd_and_hms(2026, 8, 29, 13, 45, 0)
        .unwrap();
    vec![
        Profile {
            name: "机场订阅".to_string(),
            path: PathBuf::from("/home/demo/.config/musicfrog-infiltrator/profiles/机场订阅.yaml"),
            active: true,
            subscription_url: Some(
                "https://sub.example.com/api/v1/client/subscribe?token=demo-token".to_string(),
            ),
            auto_update_enabled: true,
            update_interval_hours: Some(24),
            last_updated: Some(updated),
            next_update: Some(updated + chrono::Duration::hours(24)),
        },
        Profile {
            name: "本地配置".to_string(),
            path: PathBuf::from("/home/demo/.config/musicfrog-infiltrator/profiles/本地配置.yaml"),
            active: false,
            subscription_url: None,
            auto_update_enabled: false,
            update_interval_hours: None,
            last_updated: Some(updated),
            next_update: None,
        },
        Profile {
            name: "备用线路".to_string(),
            path: PathBuf::from("/home/demo/.config/musicfrog-infiltrator/profiles/备用线路.yaml"),
            active: false,
            subscription_url: Some("https://backup.example.com/link/demo".to_string()),
            auto_update_enabled: false,
            update_interval_hours: Some(12),
            last_updated: Some(updated),
            next_update: None,
        },
    ]
}

pub(super) fn rule_providers_json_fixture() -> String {
    [
        "{",
        "  \"reject\": { \"type\": \"classical\", \"behavior\": \"domain\", \"vehicleType\": \"HTTP\", \"ruleCount\": 52345, \"updatedAt\": \"2026-08-29T12:00:00.000000+08:00\" },",
        "  \"icloud\": { \"type\": \"classical\", \"behavior\": \"domain\", \"vehicleType\": \"HTTP\", \"ruleCount\": 1482, \"updatedAt\": \"2026-08-29T12:00:00.000000+08:00\" },",
        "  \"google\": { \"type\": \"classical\", \"behavior\": \"domain\", \"vehicleType\": \"HTTP\", \"ruleCount\": 785, \"updatedAt\": \"2026-08-29T12:00:00.000000+08:00\" },",
        "  \"cn-cidr\": { \"type\": \"classical\", \"behavior\": \"ipcidr\", \"vehicleType\": \"HTTP\", \"ruleCount\": 9412, \"updatedAt\": \"2026-08-29T12:00:00.000000+08:00\" }",
        "}",
    ]
    .join("\n")
}

pub(super) fn proxy_providers_json_fixture() -> String {
    [
        "{",
        "  \"机场订阅\": { \"type\": \"proxy\", \"vehicleType\": \"HTTP\", \"updatedAt\": \"2026-08-29T13:45:00.000000+08:00\", \"proxies\": 7 }",
        "}",
    ]
    .join("\n")
}

pub(super) fn sniffer_json_fixture() -> String {
    [
        "{",
        "  \"enable\": true,",
        "  \"parse-pure-ip\": true,",
        "  \"override-destination\": true,",
        "  \"sniff\": {",
        "    \"TLS\": { \"ports\": [443, 8443] },",
        "    \"HTTP\": { \"ports\": [80, \"8080-8880\"] },",
        "    \"QUIC\": { \"ports\": [443, 8443] }",
        "  }",
        "}",
    ]
    .join("\n")
}

pub(super) fn dns_json_fixture() -> String {
    [
        "{",
        "  \"enable\": true,",
        "  \"ipv6\": true,",
        "  \"cache\": true,",
        "  \"use-hosts\": true,",
        "  \"use-system-hosts\": true,",
        "  \"enhanced-mode\": \"fake-ip\",",
        "  \"fake-ip-range\": \"198.18.0.1/16\",",
        "  \"nameserver\": [\"223.5.5.5\", \"119.29.29.29\", \"https://doh.pub/dns-query\"],",
        "  \"fallback\": [\"8.8.8.8\", \"1.1.1.1\"]",
        "}",
    ]
    .join("\n")
}

pub(super) fn fake_ip_json_fixture() -> String {
    [
        "{",
        "  \"fake-ip-range\": \"198.18.0.1/16\",",
        "  \"fake-ip-filter\": [\"*.lan\", \"*.local\"],",
        "  \"store-fake-ip\": true",
        "}",
    ]
    .join("\n")
}

pub(super) fn tun_json_fixture() -> String {
    [
        "{",
        "  \"enable\": false,",
        "  \"stack\": \"gvisor\",",
        "  \"mtu\": 9001,",
        "  \"dns-hijack\": [\"any:53\"],",
        "  \"auto-route\": true,",
        "  \"auto-detect-interface\": true,",
        "  \"strict-route\": false",
        "}",
    ]
    .join("\n")
}

pub(super) fn profile_yaml_fixture() -> String {
    [
        "# MusicFrog Infiltrator 演示配置 (demo fixture)",
        "mixed-port: 7890",
        "allow-lan: false",
        "mode: rule",
        "log-level: info",
        "external-controller: 127.0.0.1:9090",
        "",
        "proxies:",
        "  - name: \"香港 IEPL-01\"",
        "    type: ss",
        "    server: hk-iepl-01.example.com",
        "    port: 8443",
        "    cipher: aes-256-gcm",
        "    password: \"demo-password\"",
        "    udp: true",
        "",
        "proxy-groups:",
        "  - name: \"节点选择\"",
        "    type: select",
        "    proxies:",
        "      - 香港 IEPL-01",
        "      - DMIT",
        "      - 自动选择",
        "",
        "rules:",
        "  - DOMAIN-SUFFIX,google.com,节点选择",
        "  - GEOIP,CN,DIRECT",
        "  - MATCH,漏网之鱼",
        "",
    ]
    .join("\n")
}
