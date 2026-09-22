//! Deterministic demo fixture for the Bevy Connections page.
//!
//! Kept out of the page module so the scene constructors stay inside the line
//! budget. One row deliberately crosses the shared 5 MB/s high-throughput
//! threshold so the DUAL-13-10 pulse and the DUAL-13-12 rate sort are visible
//! in demo compositions.

use crate::pages::connections::{ConnectionItem, ConnectionsProjection};

impl ConnectionsProjection {
    /// Believable demo fixture for the Connections page.
    pub fn demo() -> Self {
        Self {
            total_connections: 4,
            total_upload_bytes: 14_200_000,
            total_download_bytes: 88_900_000,
            stream_phase: infiltrator_contract::connection::ConnectionStreamPhase::Live,
            connections: vec![
                ConnectionItem {
                    id: "c-1".to_owned(),
                    host: "api.github.com:443".to_owned(),
                    process: "git (pid: 14238)".to_owned(),
                    rule: "DOMAIN-SUFFIX github.com".to_owned(),
                    rule_payload: "github.com".to_owned(),
                    network: "tcp".to_owned(),
                    source_ip: "192.168.1.20".to_owned(),
                    source_port: "51432".to_owned(),
                    destination_ip: "140.82.121.5".to_owned(),
                    destination_port: "443".to_owned(),
                    chain: "节点选择 -> 🇭🇰 香港 01".to_owned(),
                    chains: vec!["节点选择".to_owned(), "🇭🇰 香港 01".to_owned()],
                    upload_bps: 24_000.0,
                    download_bps: 180_000.0,
                    upload_total: 1_200_000,
                    download_total: 12_400_000,
                },
                ConnectionItem {
                    id: "c-2".to_owned(),
                    host: "manifest.googlevideo.com:443".to_owned(),
                    process: "chrome (pid: 8912)".to_owned(),
                    rule: "GEOSITE youtube".to_owned(),
                    rule_payload: "youtube".to_owned(),
                    network: "tcp".to_owned(),
                    source_ip: "192.168.1.20".to_owned(),
                    source_port: "51888".to_owned(),
                    destination_ip: "142.250.71.174".to_owned(),
                    destination_port: "443".to_owned(),
                    chain: "国外媒体 -> 🇸🇬 新加坡 01".to_owned(),
                    chains: vec!["国外媒体".to_owned(), "🇸🇬 新加坡 01".to_owned()],
                    upload_bps: 8_500.0,
                    download_bps: 8_500_000.0,
                    upload_total: 450_000,
                    download_total: 68_000_000,
                },
                ConnectionItem {
                    id: "c-3".to_owned(),
                    host: "gateway.discord.gg:443".to_owned(),
                    process: "Discord (pid: 11024)".to_owned(),
                    rule: "DOMAIN-SUFFIX discord.gg".to_owned(),
                    rule_payload: "discord.gg".to_owned(),
                    network: "tcp".to_owned(),
                    source_ip: "192.168.1.20".to_owned(),
                    source_port: "52004".to_owned(),
                    destination_ip: "162.159.128.233".to_owned(),
                    destination_port: "443".to_owned(),
                    chain: "节点选择 -> 🇭🇰 香港 01".to_owned(),
                    chains: vec!["节点选择".to_owned(), "🇭🇰 香港 01".to_owned()],
                    upload_bps: 1_200.0,
                    download_bps: 3_400.0,
                    upload_total: 890_000,
                    download_total: 4_200_000,
                },
                ConnectionItem {
                    id: "c-4".to_owned(),
                    host: "119.29.29.29:53".to_owned(),
                    process: "systemd-resolved".to_owned(),
                    rule: "GEOIP CN".to_owned(),
                    rule_payload: "CN".to_owned(),
                    network: "udp".to_owned(),
                    source_ip: "127.0.0.1".to_owned(),
                    source_port: "39118".to_owned(),
                    destination_ip: "119.29.29.29".to_owned(),
                    destination_port: "53".to_owned(),
                    chain: "DIRECT".to_owned(),
                    chains: vec!["DIRECT".to_owned()],
                    upload_bps: 0.0,
                    download_bps: 0.0,
                    upload_total: 12_000,
                    download_total: 34_000,
                },
            ],
        }
    }
}
