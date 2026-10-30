//! Presentation projections for user-owned local proxy configurations (FLPARITY-037).
//!
//! Maps domain local proxy snapshots into unified view-models for desktop UIs
//! (Iced, Bevy, Slint) and Android Compose surfaces.

use infiltrator_contract::local_proxy::{LocalProxyNodeSummary, LocalProxySnapshot};

/// Projected row for a single local proxy node in multi-surface proxy lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalProxyRowProjection {
    pub id: String,
    pub name: String,
    pub protocol_badge: String,
    pub endpoint_display: String,
    pub source_display: String,
    pub security_display: String,
    pub has_tls: bool,
    pub has_secret: bool,
}

/// Projected page/panel model for the local proxy collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalProxyPageProjection {
    pub rows: Vec<LocalProxyRowProjection>,
    pub total_count: usize,
    pub count_headline: String,
    pub is_empty: bool,
    pub empty_placeholder: String,
}

/// Project a `LocalProxySnapshot` into a presentation-layer `LocalProxyPageProjection`.
pub fn project_local_proxies(snapshot: &LocalProxySnapshot) -> LocalProxyPageProjection {
    let rows: Vec<_> = snapshot.nodes.iter().map(project_local_proxy_row).collect();
    let total_count = snapshot.total_count;
    let is_empty = total_count == 0;
    let count_headline = if is_empty {
        "暂无本地代理节点".to_owned()
    } else {
        format!("本地代理 ({total_count})")
    };
    let empty_placeholder = "可从已订阅节点复制加入，或手动添加本地代理配置".to_owned();

    LocalProxyPageProjection {
        rows,
        total_count,
        count_headline,
        is_empty,
        empty_placeholder,
    }
}

fn project_local_proxy_row(node: &LocalProxyNodeSummary) -> LocalProxyRowProjection {
    let endpoint_display = format!("{}:{}", node.server, node.port);
    let source_display = if let Some(ref pid) = node.source_profile_id {
        format!("源自订阅: {pid}")
    } else {
        "本地自建".to_owned()
    };
    let security_display = if node.tls {
        if let Some(ref sni) = node.sni {
            format!("TLS ({sni})")
        } else {
            "TLS".to_owned()
        }
    } else if let Some(ref cipher) = node.cipher {
        cipher.clone()
    } else {
        "明文/直连".to_owned()
    };

    LocalProxyRowProjection {
        id: node.id.clone(),
        name: node.name.clone(),
        protocol_badge: node.protocol.badge().to_owned(),
        endpoint_display,
        source_display,
        security_display,
        has_tls: node.tls,
        has_secret: node.has_secret,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::local_proxy::LocalProxyProtocol;

    #[test]
    fn project_empty_local_proxies() {
        let snapshot = LocalProxySnapshot::default();
        let proj = project_local_proxies(&snapshot);
        assert!(proj.is_empty);
        assert_eq!(proj.total_count, 0);
        assert_eq!(proj.count_headline, "暂无本地代理节点");
    }

    #[test]
    fn project_populated_local_proxies() {
        let snapshot = LocalProxySnapshot {
            nodes: vec![LocalProxyNodeSummary {
                id: "p1".into(),
                name: "HK Premium".into(),
                protocol: LocalProxyProtocol::Shadowsocks,
                server: "hk.example.com".into(),
                port: 8388,
                has_secret: true,
                cipher: Some("chacha20-ietf-poly1305".into()),
                network: Some("tcp".into()),
                tls: false,
                sni: None,
                source_profile_id: Some("sub-123".into()),
                added_at_epoch_ms: 1000,
            }],
            total_count: 1,
            revision: 1,
            last_updated_epoch_ms: 1000,
        };

        let proj = project_local_proxies(&snapshot);
        assert!(!proj.is_empty);
        assert_eq!(proj.total_count, 1);
        assert_eq!(proj.count_headline, "本地代理 (1)");
        assert_eq!(proj.rows[0].endpoint_display, "hk.example.com:8388");
        assert_eq!(proj.rows[0].protocol_badge, "SS");
        assert_eq!(proj.rows[0].source_display, "源自订阅: sub-123");
        assert_eq!(proj.rows[0].security_display, "chacha20-ietf-poly1305");
        assert!(proj.rows[0].has_secret);
    }
}
