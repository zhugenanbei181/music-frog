use super::*;

    fn make_test_conn(
        id: &str,
        host: &str,
        source_ip: &str,
        process: &str,
        chains: Vec<String>,
        up: u64,
        down: u64,
    ) -> mihomo_api::types::Connection {
        mihomo_api::types::Connection {
            id: id.to_string(),
            metadata: mihomo_api::types::ConnectionMetadata {
                network: "tcp".to_string(),
                connection_type: "TLS".to_string(),
                source_ip: source_ip.to_string(),
                destination_ip: "1.1.1.1".to_string(),
                source_port: "50000".to_string(),
                destination_port: "443".to_string(),
                host: host.to_string(),
                dns_mode: "fake-ip".to_string(),
                process_path: process.to_string(),
                special_proxy: String::new(),
            },
            upload: up,
            download: down,
            start: "2026-09-01T12:00:00Z".to_string(),
            rule: "Match".to_string(),
            rule_payload: String::new(),
            chains,
        }
    }

    #[test]
    fn test_smooth_path_empty_and_single() {
        let empty_pts: Vec<Point> = vec![];
        let _path = build_smooth_path(&empty_pts);

        let single_pt = vec![Point::new(10.0, 20.0)];
        let _path_single = build_smooth_path(&single_pt);
    }

    #[test]
    fn test_smooth_path_multiple_points() {
        let pts = vec![
            Point::new(0.0, 100.0),
            Point::new(10.0, 80.0),
            Point::new(20.0, 50.0),
            Point::new(30.0, 20.0),
        ];
        let _path = build_smooth_path(&pts);
        let _area = build_smooth_area(&pts, 120.0);
    }

    #[test]
    fn test_host_traffic_rankings_calculation() {
        let (mut state, _) = AppState::new();
        let mut snapshot = mihomo_api::types::ConnectionSnapshot::default();
        let conn1 = make_test_conn("conn-1", "google.com", "", "", vec![], 100, 900);
        let conn2 = make_test_conn("conn-2", "youtube.com", "", "", vec![], 1000, 3000);
        snapshot.connections = vec![conn1, conn2];
        state.diag.connections = Some(snapshot);

        let ranks = compute_host_rankings(&state);
        assert_eq!(ranks.len(), 2);
        assert_eq!(ranks[0].host, "youtube.com");
        assert_eq!(ranks[0].total, 4000);
        assert_eq!(ranks[0].share_percent, 80.0);
        assert_eq!(ranks[1].host, "google.com");
        assert_eq!(ranks[1].total, 1000);
        assert_eq!(ranks[1].share_percent, 20.0);
    }

    #[test]
    fn test_device_rankings_calculation() {
        let (mut state, _) = AppState::new();
        let mut snapshot = mihomo_api::types::ConnectionSnapshot::default();
        let conn1 = make_test_conn("c1", "", "192.168.1.10", "", vec![], 200, 800);
        let conn2 = make_test_conn("c2", "", "192.168.1.20", "", vec![], 500, 2500);
        snapshot.connections = vec![conn1, conn2];
        state.diag.connections = Some(snapshot);

        let ranks = compute_device_rankings(&state);
        assert_eq!(ranks.len(), 2);
        assert_eq!(ranks[0].host, "192.168.1.20");
        assert_eq!(ranks[0].total, 3000);
        assert_eq!(ranks[0].share_percent, 75.0);
        assert_eq!(ranks[1].host, "192.168.1.10");
        assert_eq!(ranks[1].total, 1000);
        assert_eq!(ranks[1].share_percent, 25.0);
    }

    #[test]
    fn test_proxy_rankings_calculation() {
        let (mut state, _) = AppState::new();
        let mut snapshot = mihomo_api::types::ConnectionSnapshot::default();
        let conn1 = make_test_conn("c1", "", "", "", vec!["HK-Node-01".to_string()], 1000, 1000);
        let conn2 = make_test_conn("c2", "", "", "", vec!["US-Node-02".to_string()], 2000, 6000);
        snapshot.connections = vec![conn1, conn2];
        state.diag.connections = Some(snapshot);

        let ranks = compute_proxy_rankings(&state);
        assert_eq!(ranks.len(), 2);
        assert_eq!(ranks[0].host, "US-Node-02");
        assert_eq!(ranks[0].total, 8000);
        assert_eq!(ranks[0].share_percent, 80.0);
        assert_eq!(ranks[1].host, "HK-Node-01");
        assert_eq!(ranks[1].total, 2000);
        assert_eq!(ranks[1].share_percent, 20.0);
    }

    #[test]
    fn test_process_rankings_calculation() {
        let (mut state, _) = AppState::new();
        let mut snapshot = mihomo_api::types::ConnectionSnapshot::default();
        let conn1 = make_test_conn("c1", "", "", "/usr/bin/curl", vec![], 300, 700);
        let conn2 = make_test_conn(
            "c2",
            "",
            "",
            "C:\\Program Files\\Firefox\\firefox.exe",
            vec![],
            3000,
            6000,
        );
        snapshot.connections = vec![conn1, conn2];
        state.diag.connections = Some(snapshot);

        let ranks = compute_process_rankings(&state);
        assert_eq!(ranks.len(), 2);
        assert_eq!(ranks[0].host, "firefox");
        assert_eq!(ranks[0].total, 9000);
        assert_eq!(ranks[0].share_percent, 90.0);
        assert_eq!(ranks[1].host, "curl");
        assert_eq!(ranks[1].total, 1000);
        assert_eq!(ranks[1].share_percent, 10.0);
    }

    #[test]
    fn test_dimension_rankings_dispatch() {
        let (mut state, _) = AppState::new();
        let mut snapshot = mihomo_api::types::ConnectionSnapshot::default();
        let conn = make_test_conn(
            "c1",
            "example.org",
            "10.0.0.1",
            "/bin/git",
            vec!["ProxyGroup".to_string()],
            100,
            400,
        );
        snapshot.connections = vec![conn];
        state.diag.connections = Some(snapshot);

        let dom_ranks = compute_dimension_rankings(&state, TrafficDimension::Domains);
        assert_eq!(dom_ranks[0].host, "example.org");

        let dev_ranks = compute_dimension_rankings(&state, TrafficDimension::Devices);
        assert_eq!(dev_ranks[0].host, "10.0.0.1");

        let prx_ranks = compute_dimension_rankings(&state, TrafficDimension::Proxies);
        assert_eq!(prx_ranks[0].host, "ProxyGroup");

        let prc_ranks = compute_dimension_rankings(&state, TrafficDimension::Processes);
        assert_eq!(prc_ranks[0].host, "git");
    }

    #[test]
    fn test_traffic_dimension_labels_and_icons() {
        assert_eq!(TrafficDimension::Domains.label(&Lang("zh-CN")), "域名 (Domains)");
        assert_eq!(TrafficDimension::Domains.label(&Lang("en-US")), "Domains");
        assert_eq!(TrafficDimension::Devices.label(&Lang("zh-CN")), "设备 (Devices)");
        assert_eq!(TrafficDimension::Devices.label(&Lang("en-US")), "Devices");
        assert_eq!(TrafficDimension::Proxies.label(&Lang("zh-CN")), "代理 (Proxies)");
        assert_eq!(TrafficDimension::Proxies.label(&Lang("en-US")), "Proxies");
        assert_eq!(TrafficDimension::Processes.label(&Lang("zh-CN")), "进程 (Processes)");
        assert_eq!(TrafficDimension::Processes.label(&Lang("en-US")), "Processes");
        assert_eq!(TrafficDimension::Domains.icon(), Icon::Globe);
        assert_eq!(TrafficDimension::Devices.icon(), Icon::Server);
        assert_eq!(TrafficDimension::Proxies.icon(), Icon::Zap);
        assert_eq!(TrafficDimension::Processes.icon(), Icon::Code2);
    }

    #[test]
    fn test_extract_process_name() {
        assert_eq!(extract_process_name("/usr/bin/curl"), "curl");
        assert_eq!(extract_process_name("C:\\app.exe"), "app");
        assert_eq!(extract_process_name(""), "");
    }
