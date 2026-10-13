//! Real-process evidence for the Geo update trigger's accepted semantics.
//!
//! The profile binds every geox URL to the smoke harness's loopback server.
//! The test verifies that the production controller client receives an
//! accepted response from the running Mihomo process. The asynchronous
//! database fetch and installation are deliberately outside this evidence.

use mihomo_api::client::MihomoClient;
use std::env;
use std::path::PathBuf;
use tokio::fs;
use url::Url;

fn required_environment(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the explicit Mihomo smoke"))
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo real-process smoke"]
async fn geo_update_trigger_is_accepted_under_loopback_only_geox_configuration() {
    let controller_url = required_environment("MIHOMO_REAL_CONTROLLER_URL");
    let controller_secret = required_environment("MIHOMO_REAL_CONTROLLER_SECRET");
    let expected_version = required_environment("MIHOMO_REAL_VERSION");
    let profile_path = PathBuf::from(required_environment("MIHOMO_REAL_PROFILE_PATH"));
    let fixture_origin = required_environment("MIHOMO_REAL_GEO_FIXTURE_ORIGIN");

    let parsed_origin = Url::parse(&fixture_origin).expect("loopback fixture origin parses");
    assert_eq!(parsed_origin.scheme(), "http");
    assert!(
        parsed_origin
            .host_str()
            .and_then(|host| host.parse::<std::net::IpAddr>().ok())
            .is_some_and(|address| address.is_loopback())
    );

    let profile_source = fs::read_to_string(&profile_path)
        .await
        .expect("running profile config is readable");
    for suffix in ["/geoip.dat", "/geosite.dat", "/geoip.metadb", "/asn.mmdb"] {
        assert!(
            profile_source.contains(&format!("{fixture_origin}{suffix}")),
            "every configured geox URL must target the local fixture: {suffix}"
        );
    }
    assert!(profile_source.contains("geo-auto-update: false"));
    assert!(profile_source.contains("IP-CIDR,127.0.0.0/8,DIRECT,no-resolve"));
    assert!(profile_source.contains("MATCH,REJECT"));

    let client = MihomoClient::new(&controller_url, Some(controller_secret.clone()))
        .expect("construct the production Mihomo controller client");
    let observed_version = client
        .get_version()
        .await
        .expect("read the running Mihomo version");
    assert_eq!(observed_version.version, expected_version);

    client
        .upgrade_geo()
        .await
        .expect("the running Mihomo controller accepts POST /upgrade/geo");
}
