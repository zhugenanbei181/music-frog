/// Mihomo API 响应类型反序列化测试
///
/// `types.rs` 定义了从 Mihomo REST API 响应中解析的所有数据结构。
/// 这些结构的 serde 字段名（snake_case ↔ kebab-case/camelCase）与
/// Mihomo 实际 JSON 格式必须严格匹配，否则会导致运行时数据丢失。
#[cfg(test)]
mod tests {
    use crate::types::{
        ConfigResponse, Connection, ConnectionSnapshot, MemoryData, Rule, TrafficData, Version,
    };

    // ──────────────────────────────────────────────
    // ConfigResponse：核心配置读取
    // ──────────────────────────────────────────────

    #[test]
    fn config_response_deserializes_kebab_case_port_fields() {
        // Mihomo 使用 kebab-case JSON 字段（mixed-port, socks-port 等），
        // serde rename 必须与 API 格式对齐
        let json = r#"{
            "port": 7890,
            "socks-port": 7891,
            "redir-port": 0,
            "tproxy-port": 0,
            "mixed-port": 7892,
            "mode": "rule",
            "log-level": "info",
            "allow-lan": false
        }"#;
        let config: ConfigResponse = serde_json::from_str(json).unwrap();
        assert_eq!(config.port, 7890);
        assert_eq!(config.socks_port, 7891);
        assert_eq!(config.mixed_port, 7892);
        assert_eq!(config.mode, "rule");
        assert_eq!(config.log_level, "info");
        assert!(!config.allow_lan);
    }

    #[test]
    fn config_response_mode_reflects_current_proxy_routing_strategy() {
        // mode 字段决定流量路由策略：rule / global / direct
        // 该字段直接影响 UI 中代理模式的显示与切换
        for mode in ["rule", "global", "direct"] {
            let json = format!(
                r#"{{"port":7890,"socks-port":0,"redir-port":0,"tproxy-port":0,"mixed-port":0,"mode":"{}","log-level":"info","allow-lan":false}}"#,
                mode
            );
            let config: ConfigResponse = serde_json::from_str(&json).unwrap();
            assert_eq!(config.mode, mode, "代理模式 {mode} 应能正确反序列化");
        }
    }

    // ──────────────────────────────────────────────
    // TrafficData：实时流量数据
    // ──────────────────────────────────────────────

    #[test]
    fn traffic_data_deserializes_up_and_down_as_bytes_per_second() {
        // up/down 字段为每秒字节数，用于实时流量图表展示
        let json = r#"{"up": 102400, "down": 204800}"#;
        let traffic: TrafficData = serde_json::from_str(json).unwrap();
        assert_eq!(traffic.up, 102400);
        assert_eq!(traffic.down, 204800);
    }

    #[test]
    fn traffic_data_defaults_to_zero_when_idle() {
        let json = r#"{"up": 0, "down": 0}"#;
        let traffic: TrafficData = serde_json::from_str(json).unwrap();
        assert_eq!(traffic.up, 0);
        assert_eq!(traffic.down, 0);
    }

    // ──────────────────────────────────────────────
    // Connection / ConnectionSnapshot：连接追踪
    // ──────────────────────────────────────────────

    #[test]
    fn connection_metadata_deserializes_camel_case_fields() {
        // Mihomo 连接元数据使用 camelCase（sourceIP, destinationPort 等）
        let json = r#"{
            "id": "conn-1",
            "metadata": {
                "network": "tcp",
                "type": "HTTPS",
                "sourceIP": "192.168.1.100",
                "destinationIP": "1.1.1.1",
                "sourcePort": "54321",
                "destinationPort": "443",
                "host": "example.com",
                "dnsMode": "normal",
                "processPath": "/usr/bin/curl"
            },
            "uploadTotal": 1024,
            "downloadTotal": 4096,
            "start": "2024-01-01T00:00:00Z",
            "rule": "DOMAIN-SUFFIX,example.com,PROXY",
            "rulePayload": "example.com"
        }"#;
        let conn: Connection = serde_json::from_str(json).unwrap();
        assert_eq!(conn.id, "conn-1");
        assert_eq!(conn.metadata.source_ip, "192.168.1.100");
        assert_eq!(conn.metadata.destination_ip, "1.1.1.1");
        assert_eq!(conn.metadata.destination_port, "443");
        assert_eq!(conn.metadata.host, "example.com");
        assert_eq!(conn.metadata.process_path, "/usr/bin/curl");
        assert_eq!(conn.upload, 1024);
        assert_eq!(conn.download, 4096);
        assert_eq!(conn.rule, "DOMAIN-SUFFIX,example.com,PROXY");
    }

    #[test]
    fn connection_snapshot_with_null_connections_field_deserializes_as_empty_vec() {
        // Mihomo 有时返回 `"connections": null`，必须容忍并作为空列表处理，
        // 否则 serde 会报错导致连接面板崩溃
        let json = r#"{
            "downloadTotal": 1000,
            "uploadTotal": 500,
            "connections": null
        }"#;
        let snapshot: ConnectionSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.download_total, 1000);
        assert_eq!(snapshot.upload_total, 500);
        assert!(
            snapshot.connections.is_empty(),
            "null connections 字段应被视为空列表而非反序列化错误"
        );
    }

    #[test]
    fn connection_snapshot_accumulates_total_traffic_across_all_connections() {
        // downloadTotal / uploadTotal 是所有连接的累计流量，不是瞬时速率
        let json = r#"{
            "downloadTotal": 10485760,
            "uploadTotal": 2097152,
            "connections": []
        }"#;
        let snapshot: ConnectionSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.download_total, 10_485_760); // 10 MB
        assert_eq!(snapshot.upload_total, 2_097_152); // 2 MB
    }

    // ──────────────────────────────────────────────
    // Version：内核版本信息
    // ──────────────────────────────────────────────

    #[test]
    fn version_response_carries_premium_flag() {
        // premium 字段区分社区版和 Meta 内核，影响 TUN / fake-ip 等功能的可用性
        let json = r#"{"version": "v1.18.0", "premium": false}"#;
        let ver: Version = serde_json::from_str(json).unwrap();
        assert_eq!(ver.version, "v1.18.0");
        assert!(!ver.premium);

        let json_premium = r#"{"version": "v1.18.0-Meta", "premium": true}"#;
        let ver_premium: Version = serde_json::from_str(json_premium).unwrap();
        assert!(ver_premium.premium, "Meta 内核应标记 premium=true");
    }

    // ──────────────────────────────────────────────
    // Rule：规则列表
    // ──────────────────────────────────────────────

    #[test]
    fn rule_deserializes_type_and_payload_and_proxy_target() {
        // 规则列表用于展示当前配置的路由规则及其命中统计
        let json = r#"{
            "type": "DOMAIN-SUFFIX",
            "payload": "google.com",
            "proxy": "PROXY",
            "size": 1234
        }"#;
        let rule: Rule = serde_json::from_str(json).unwrap();
        assert_eq!(rule.rule_type, "DOMAIN-SUFFIX");
        assert_eq!(rule.payload, "google.com");
        assert_eq!(rule.proxy, "PROXY");
        assert_eq!(rule.size, 1234);
    }

    // ──────────────────────────────────────────────
    // MemoryData：内存使用
    // ──────────────────────────────────────────────

    #[test]
    fn memory_data_deserializes_inuse_and_oslimit_renamed_fields() {
        // Mihomo 返回 "inuse" 和 "oslimit"，serde rename 必须对应
        let json = r#"{"inuse": 52428800, "oslimit": 4294967296}"#;
        let mem: MemoryData = serde_json::from_str(json).unwrap();
        assert_eq!(mem.in_use, 52_428_800); // 50 MB
        assert_eq!(mem.os_limit, 4_294_967_296); // 4 GB
    }
}
