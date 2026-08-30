//! Demo proxy-table fixtures: provider groups, exit nodes with mixed latency
//! tiers and the deterministic `filtered_groups` ordering.

use mihomo_api::proxy::types::{
    Hysteria2, Proxy, ProxyBase, ProxyGroup, ProxyHistory, Shadowsocks, Trojan, Vmess,
};
use std::collections::HashMap;

/// Proxy group name constants shared between the table builder and the
/// runtime-selection fixture.
const G_MAIN: &str = "节点选择";
const G_DIRECT: &str = "全球直连";
const G_CAMPUS: &str = "校园网";
const G_AI: &str = "AI 服务";
const G_GAME: &str = "游戏平台";
const G_AUTO: &str = "自动选择";
const G_GLOBAL: &str = "GLOBAL";

const N_HK1: &str = "香港 IEPL-01";
const N_HK2: &str = "香港 IEPL-02";
const N_JP: &str = "日本 NTT";
const N_SG: &str = "新加坡 BGP";
const N_US: &str = "美国 CN2";
const N_DMIT: &str = "DMIT";
const N_ZGO: &str = "ZGO";

/// Build the demo proxy map plus a deterministic `filtered_groups` ordering
/// (GLOBAL last so the capture always sees the business groups first).
pub(super) fn demo_proxy_tables() -> (HashMap<String, Proxy>, Vec<(String, Vec<String>)>) {
    let mut proxies: HashMap<String, Proxy> = HashMap::new();

    let mut insert_group = |name: &str, now: &str, all: Vec<&str>| {
        proxies.insert(
            name.to_string(),
            Proxy::Selector(ProxyGroup {
                name: name.to_string(),
                now: now.to_string(),
                all: all.iter().map(|s| s.to_string()).collect(),
                history: vec![ProxyHistory {
                    time: "2026-08-29T15:30:00.000000+08:00".to_string(),
                    delay: 0,
                }],
            }),
        );
    };

    insert_group(
        G_MAIN,
        N_HK1,
        vec![N_HK1, N_DMIT, N_HK2, N_SG, N_JP, N_US, N_ZGO],
    );
    insert_group(G_DIRECT, "DIRECT", vec!["DIRECT", N_HK1, N_DMIT]);
    insert_group(
        G_CAMPUS,
        "DIRECT",
        vec!["DIRECT", "REJECT", N_HK2, N_DMIT],
    );
    insert_group(G_AI, N_US, vec![N_US, N_SG, N_JP, N_DMIT]);
    insert_group(G_GAME, N_JP, vec![N_JP, N_HK1, N_HK2, N_US]);
    insert_group(
        G_GLOBAL,
        G_MAIN,
        vec![G_MAIN, G_DIRECT, G_CAMPUS, G_AI, G_GAME, G_AUTO],
    );
    proxies.insert(
        G_AUTO.to_string(),
        Proxy::URLTest(ProxyGroup {
            name: G_AUTO.to_string(),
            now: N_HK1.to_string(),
            all: vec![
                N_HK1.to_string(),
                N_DMIT.to_string(),
                N_HK2.to_string(),
                N_SG.to_string(),
                N_JP.to_string(),
            ],
            history: vec![ProxyHistory {
                time: "2026-08-29T15:30:00.000000+08:00".to_string(),
                delay: 0,
            }],
        }),
    );

    // Node fixtures with mixed latency tiers so every badge color renders:
    // 148 (fast) / 189 / 233 / 254 / 312 / 512 (slow) / untested ("—").
    let nodes: Vec<Proxy> = vec![
        ss_node(N_HK1, 148, "hk-iepl-01.example.com", 8443),
        vmess_node(N_DMIT, 189, "dmit.example.com", 443),
        ss_node(N_HK2, 233, "hk-iepl-02.example.com", 8443),
        trojan_node(N_SG, 254, "sg-bgp.example.com", 443),
        vmess_node(N_JP, 312, "jp-ntt.example.com", 443),
        trojan_node(N_US, 512, "us-cn2.example.com", 443),
        hysteria2_node(N_ZGO, "zgo.example.com", 36712),
        Proxy::Direct(direct_base("DIRECT")),
        Proxy::Reject(reject_base("REJECT")),
    ];
    for node in nodes {
        let name = node.name().to_string();
        proxies.insert(name, node);
    }

    let filtered_groups: Vec<(String, Vec<String>)> = vec![
        (
            G_MAIN.to_string(),
            vec![
                N_HK1.to_string(),
                N_DMIT.to_string(),
                N_HK2.to_string(),
                N_SG.to_string(),
                N_JP.to_string(),
                N_US.to_string(),
                N_ZGO.to_string(),
            ],
        ),
        (
            G_DIRECT.to_string(),
            vec!["DIRECT".to_string(), N_HK1.to_string(), N_DMIT.to_string()],
        ),
        (
            G_CAMPUS.to_string(),
            vec![
                "DIRECT".to_string(),
                "REJECT".to_string(),
                N_HK2.to_string(),
                N_DMIT.to_string(),
            ],
        ),
        (
            G_AI.to_string(),
            vec![
                N_US.to_string(),
                N_SG.to_string(),
                N_JP.to_string(),
                N_DMIT.to_string(),
            ],
        ),
        (
            G_GAME.to_string(),
            vec![
                N_JP.to_string(),
                N_HK1.to_string(),
                N_HK2.to_string(),
                N_US.to_string(),
            ],
        ),
        (
            G_AUTO.to_string(),
            vec![
                N_HK1.to_string(),
                N_DMIT.to_string(),
                N_HK2.to_string(),
                N_SG.to_string(),
                N_JP.to_string(),
            ],
        ),
        (
            G_GLOBAL.to_string(),
            vec![
                G_MAIN.to_string(),
                G_DIRECT.to_string(),
                G_CAMPUS.to_string(),
                G_AI.to_string(),
                G_GAME.to_string(),
                G_AUTO.to_string(),
            ],
        ),
    ];

    (proxies, filtered_groups)
}

/// A tested node base with one historical sample.
fn node_base(name: &str, delay: u32) -> ProxyBase {
    ProxyBase {
        name: name.to_string(),
        udp: true,
        history: vec![ProxyHistory {
            time: "2026-08-29T15:31:00.000000+08:00".to_string(),
            delay,
        }],
        alive: true,
        delay: Some(delay),
    }
}

/// An untested node base (no history) — renders the "—" latency tier.
fn untested_base(name: &str) -> ProxyBase {
    ProxyBase {
        name: name.to_string(),
        udp: true,
        history: Vec::new(),
        alive: true,
        delay: None,
    }
}

fn ss_node(name: &str, delay: u32, server: &str, port: u16) -> Proxy {
    Proxy::Shadowsocks(Shadowsocks {
        base: node_base(name, delay),
        server: server.to_string(),
        port,
        cipher: "aes-256-gcm".to_string(),
        plugin: None,
        plugin_opts: None,
    })
}

fn vmess_node(name: &str, delay: u32, server: &str, port: u16) -> Proxy {
    Proxy::Vmess(Vmess {
        base: node_base(name, delay),
        server: server.to_string(),
        port,
        uuid: "88888888-4444-4444-4444-cccccccccccc".to_string(),
        ..Vmess::default()
    })
}

fn trojan_node(name: &str, delay: u32, server: &str, port: u16) -> Proxy {
    Proxy::Trojan(Trojan {
        base: node_base(name, delay),
        server: server.to_string(),
        port,
        ..Trojan::default()
    })
}

fn hysteria2_node(name: &str, server: &str, port: u16) -> Proxy {
    Proxy::Hysteria2(Hysteria2 {
        base: untested_base(name),
        server: server.to_string(),
        port,
        ..Hysteria2::default()
    })
}

fn direct_base(name: &str) -> mihomo_api::proxy::types::Direct {
    mihomo_api::proxy::types::Direct {
        base: untested_base(name),
    }
}

fn reject_base(name: &str) -> mihomo_api::proxy::types::Reject {
    mihomo_api::proxy::types::Reject {
        base: untested_base(name),
    }
}
