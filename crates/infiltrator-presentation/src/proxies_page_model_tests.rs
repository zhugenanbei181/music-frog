//! test-intent: behavior
use crate::filter_query::{FilterQuery, LatencyBound};
use crate::proxies_page_model::{
    PROXY_SORT_OPTIONS, ProxiesPageModel, ProxyFilterTarget, matches_advanced_filter,
    proxy_feature_tags, proxy_flag_icon_token, proxy_name_runs, proxy_node_region,
    proxy_protocol_chip, proxy_region_label_en, proxy_region_label_zh, sort_option_at,
    sort_option_index,
};
use lurker_core_contract::outbound::details::OutboundHealthObservation;
use lurker_core_contract::outbound::preferences::{OutboundHealthFilter, OutboundSortOrder};

#[derive(Clone, Debug)]
struct Node {
    name: String,
    identity: Option<String>,
    protocol: String,
    features: Vec<String>,
    latency_ms: Option<u32>,
    udp: bool,
    favorite: bool,
}

impl Node {
    fn new(name: &str, protocol: &str, latency_ms: Option<u32>) -> Self {
        Self {
            name: name.to_owned(),
            identity: Some(format!("id-{name}")),
            protocol: protocol.to_owned(),
            features: Vec::new(),
            latency_ms,
            udp: false,
            favorite: false,
        }
    }
}

impl ProxyFilterTarget for Node {
    fn proxy_name(&self) -> &str {
        &self.name
    }

    fn proxy_identity(&self) -> Option<&str> {
        self.identity.as_deref()
    }

    fn proxy_protocol(&self) -> &str {
        &self.protocol
    }

    fn proxy_features(&self) -> &[String] {
        &self.features
    }

    fn proxy_latency_ms(&self) -> Option<u32> {
        self.latency_ms
    }

    fn proxy_supports_udp(&self) -> bool {
        self.udp
    }

    fn proxy_favorite(&self) -> bool {
        self.favorite
    }
}

#[test]
fn the_query_parses_into_one_typed_form() {
    assert_eq!(FilterQuery::parse(""), FilterQuery::MatchAll);
    assert_eq!(FilterQuery::parse("   "), FilterQuery::MatchAll);
    assert_eq!(
        FilterQuery::parse("<100"),
        FilterQuery::Latency(LatencyBound::FasterThan(100))
    );
    assert_eq!(
        FilterQuery::parse(" > 250 "),
        FilterQuery::Latency(LatencyBound::SlowerThan(250))
    );
    // A bound that is not a millisecond count stays free text, so a node
    // named `<abc` is still reachable.
    assert_eq!(
        FilterQuery::parse("<abc"),
        FilterQuery::Text("<abc".to_owned())
    );
    assert_eq!(
        FilterQuery::parse("Hong Kong"),
        FilterQuery::Text("hong kong".to_owned())
    );

    assert!(FilterQuery::parse("").matches_all());
    assert_eq!(FilterQuery::parse("").text(), "");
    assert_eq!(
        FilterQuery::parse("<100").latency_bound(),
        Some(LatencyBound::FasterThan(100))
    );
    assert!(FilterQuery::parse("hk").latency_bound().is_none());
}

#[test]
fn the_shared_rule_matches_the_name_the_identity_the_protocol_and_the_tags() {
    let mut node = Node::new("Hong Kong 01", "VLESS", Some(45));
    node.features = vec!["Reality".to_owned(), "Vision".to_owned()];

    assert!(FilterQuery::parse("").matches(&node));
    assert!(FilterQuery::parse("hong").matches(&node));
    assert!(FilterQuery::parse("id-hong").matches(&node));
    assert!(FilterQuery::parse("vless").matches(&node));
    assert!(FilterQuery::parse("reality").matches(&node));
    assert!(!FilterQuery::parse("tokyo").matches(&node));

    // The pinyin initials: the product's node names are Chinese, and the
    // initials of "香港" are "xg".
    let mut chinese = Node::new("香港 01", "VLESS", Some(45));
    chinese.features = vec!["Reality".to_owned(), "Vision".to_owned()];
    assert!(FilterQuery::parse("xg").matches(&chinese));
    assert!(!FilterQuery::parse("xk").matches(&chinese));
}

#[test]
fn the_shared_rule_answers_the_two_old_disagreements_the_same_way() {
    // Bevy used to answer "yes" for the advertised feature tag and the `ss`
    // alias while Iced answered "no".
    let mut shadowsocks = Node::new("Tokyo 01", "Shadowsocks", Some(120));
    shadowsocks.features = vec!["Reality".to_owned()];
    assert!(FilterQuery::parse("ss").matches(&shadowsocks));
    assert!(FilterQuery::parse("reality").matches(&shadowsocks));

    let hysteria = Node::new("Osaka 01", "Hysteria2", Some(80));
    assert!(FilterQuery::parse("hy2").matches(&hysteria));
    assert!(FilterQuery::parse("hy").matches(&hysteria));

    // Iced used to answer "yes" for the UDP capability while Bevy answered
    // "no".
    let mut udp_node = Node::new("Seoul 01", "VMess", Some(60));
    udp_node.udp = true;
    assert!(FilterQuery::parse("udp").matches(&udp_node));
    assert!(!FilterQuery::parse("udp").matches(&Node::new("Berlin 01", "VMess", Some(60))));
}

#[test]
fn a_latency_bound_keeps_the_measured_side_and_drops_the_unmeasured() {
    let fast = Node::new("Fast", "VLESS", Some(45));
    let slow = Node::new("Slow", "VLESS", Some(450));
    let unmeasured = Node::new("Unknown", "VLESS", None);
    let zero = Node::new("Zero", "VLESS", Some(0));

    let faster = FilterQuery::parse("<100");
    assert!(faster.matches(&fast));
    assert!(!faster.matches(&slow));
    assert!(!faster.matches(&unmeasured));
    assert!(!faster.matches(&zero));

    let slower = FilterQuery::parse(">100");
    assert!(!slower.matches(&fast));
    assert!(slower.matches(&slow));
    assert!(!slower.matches(&unmeasured));
}

#[test]
fn a_group_matches_by_name_and_never_by_a_latency_bound() {
    let mut model = ProxiesPageModel::default();

    model.edit_query("proxy");
    assert!(model.group_matches("PROXY"));
    assert!(!model.group_matches("DIRECT"));

    model.edit_query("<100");
    assert!(!model.group_matches("PROXY"));
}

#[test]
fn a_matching_group_keeps_all_members_and_a_failing_group_keeps_the_matching_ones() {
    let mut model = ProxiesPageModel::default();
    let members = vec![
        Node::new("Hong Kong 01", "VLESS", Some(45)),
        Node::new("Tokyo 01", "VMess", Some(90)),
    ];

    model.edit_query("hong");
    let visible = model.visible_members("Proxy", &members);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].proxy_name(), "Hong Kong 01");

    model.edit_query("proxy");
    let visible = model.visible_members("Proxy", &members);
    assert_eq!(visible.len(), 2);
}

#[test]
fn the_shared_order_ranks_favourites_first_and_unmeasured_latency_last() {
    let mut model = ProxiesPageModel::default();
    let mut members = vec![
        Node::new("Beta", "VLESS", Some(200)),
        Node::new("Alpha", "VLESS", Some(50)),
        Node::new("Gamma", "VLESS", None),
    ];
    members[0].favorite = true;

    model.select_sort(OutboundSortOrder::LatencyAscending);
    let ordered = model.ordered_members(&members);
    assert_eq!(
        ordered
            .iter()
            .map(|node| node.proxy_name())
            .collect::<Vec<_>>(),
        ["Beta", "Alpha", "Gamma"]
    );

    // The descending order used to put the unmeasured node first on Bevy and
    // last on Iced; the shared rule ranks it last on every surface.
    model.select_sort(OutboundSortOrder::LatencyDescending);
    let ordered = model.ordered_members(&members);
    assert_eq!(
        ordered
            .iter()
            .map(|node| node.proxy_name())
            .collect::<Vec<_>>(),
        ["Beta", "Alpha", "Gamma"]
    );

    // The name order used to ignore favourites on Bevy.
    model.select_sort(OutboundSortOrder::NameAscending);
    let ordered = model.ordered_members(&members);
    assert_eq!(
        ordered
            .iter()
            .map(|node| node.proxy_name())
            .collect::<Vec<_>>(),
        ["Beta", "Alpha", "Gamma"]
    );
}

#[test]
fn the_sort_control_has_one_shared_order_and_one_key_per_position() {
    assert_eq!(PROXY_SORT_OPTIONS.len(), 4);
    for (index, order) in PROXY_SORT_OPTIONS.iter().enumerate() {
        assert_eq!(sort_option_index(*order), index);
        assert_eq!(sort_option_at(index), *order);
    }
    assert_eq!(sort_option_at(99), OutboundSortOrder::default());
    assert_eq!(sort_option_index(OutboundSortOrder::default()), 0);
}

#[test]
fn the_health_filter_never_accepts_an_unobserved_health() {
    let mut model = ProxiesPageModel::default();
    assert!(model.keeps_health(OutboundHealthObservation::Unknown));
    assert!(model.keeps_health(OutboundHealthObservation::Unhealthy));
    assert_eq!(model.health_filter(), OutboundHealthFilter::All);

    model.select_health_filter(OutboundHealthFilter::HealthyOnly);
    assert!(model.healthy_only());
    assert!(model.keeps_health(OutboundHealthObservation::Healthy));
    // An unknown or unsupported health never satisfies `HealthyOnly`.
    assert!(!model.keeps_health(OutboundHealthObservation::Unknown));
    assert!(!model.keeps_health(OutboundHealthObservation::Unsupported));
    assert!(!model.keeps_health(OutboundHealthObservation::Unhealthy));
}

#[test]
fn the_protocol_chip_has_one_shared_spelling() {
    assert_eq!(proxy_protocol_chip("Shadowsocks"), "Shadowsocks");
    assert_eq!(proxy_protocol_chip("ss"), "Shadowsocks");
    assert_eq!(proxy_protocol_chip("hysteria2"), "Hysteria2");
    assert_eq!(proxy_protocol_chip("hy2"), "Hysteria2");
    assert_eq!(proxy_protocol_chip("vmess"), "VMess");
    assert_eq!(proxy_protocol_chip(""), "Proxy");
    assert_eq!(proxy_protocol_chip("custom"), "custom");
}

#[test]
fn the_name_highlight_follows_the_shared_match_rule() {
    let query = FilterQuery::parse("hong");
    let runs = proxy_name_runs("Hong Kong 01", &query);
    assert!(runs.iter().any(|run| run.highlighted));

    // A pinyin-only reach highlights the whole name.
    let query = FilterQuery::parse("xg");
    let runs = proxy_name_runs("香港 01", &query);
    assert!(runs.iter().all(|run| run.highlighted));

    // The empty query highlights nothing.
    let runs = proxy_name_runs("Hong Kong 01", &FilterQuery::MatchAll);
    assert!(!runs.iter().any(|run| run.highlighted));
}

#[test]
fn the_feature_tags_are_reduced_to_one_chip_each() {
    let tags = vec![
        "UDP".to_owned(),
        "udp".to_owned(),
        "  ".to_owned(),
        "IPv6".to_owned(),
        "udp".to_owned(),
    ];
    let reduced = proxy_feature_tags(&tags);
    assert_eq!(reduced, vec!["UDP".to_owned(), "IPv6".to_owned()]);
    assert!(proxy_feature_tags(&[]).is_empty());
}

#[test]
fn proxy_node_region_and_flag_token_derivation() {
    use infiltrator_contract::geo_region::GeoRegion;

    assert_eq!(proxy_node_region("🇭🇰 香港 01 · 专线"), GeoRegion::HongKong);
    assert_eq!(proxy_flag_icon_token("HK-IEPL-01"), "flag-hk");
    assert_eq!(proxy_region_label_zh("HK-IEPL-01"), "香港");
    assert_eq!(proxy_region_label_en("HK-IEPL-01"), "Hong Kong");

    assert_eq!(proxy_node_region("JP-Tokyo-02"), GeoRegion::Japan);
    assert_eq!(proxy_flag_icon_token("JP-Tokyo-02"), "flag-jp");

    assert_eq!(
        proxy_node_region("US Los Angeles Fast"),
        GeoRegion::UnitedStates
    );
    assert_eq!(proxy_flag_icon_token("US Los Angeles Fast"), "flag-us");

    assert_eq!(proxy_node_region("Singapore Anycast"), GeoRegion::Singapore);
    assert_eq!(proxy_flag_icon_token("SG-01"), "flag-sg");

    assert_eq!(proxy_node_region("Custom Node Relay"), GeoRegion::Other);
    assert_eq!(proxy_flag_icon_token("Custom Node Relay"), "flag-globe");
    assert_eq!(proxy_region_label_zh("Custom Node Relay"), "其他地区");
}

#[test]
fn advanced_proxy_filter_matching() {
    use infiltrator_contract::geo_region::GeoRegion;
    use infiltrator_contract::proxies::ProxyProtocolBadge;
    use infiltrator_contract::proxy_filter::ProxyFilterQuery;

    let node_hk = Node::new("🇭🇰 香港 IPLC 01", "ss", Some(38));
    let mut node_us = Node::new("🇺🇸 美国洛杉矶 02 - 官网重置", "vless", Some(180));
    node_us.favorite = true;
    let node_jp = Node::new("🇯🇵 日本东京 03", "trojan", Some(85));

    // 1. Empty filter matches all
    let empty_query = ProxyFilterQuery::default();
    assert!(matches_advanced_filter(&empty_query, &node_hk));
    assert!(matches_advanced_filter(&empty_query, &node_us));
    assert!(matches_advanced_filter(&empty_query, &node_jp));

    // 2. Favorite only filter
    let mut fav_query = ProxyFilterQuery::default();
    fav_query.favorite_only = true;
    assert!(!matches_advanced_filter(&fav_query, &node_hk));
    assert!(matches_advanced_filter(&fav_query, &node_us));
    assert!(!matches_advanced_filter(&fav_query, &node_jp));

    // 3. Protocol filter
    let mut proto_query = ProxyFilterQuery::default();
    proto_query.protocols = vec![ProxyProtocolBadge::Shadowsocks, ProxyProtocolBadge::Trojan];
    assert!(matches_advanced_filter(&proto_query, &node_hk));
    assert!(!matches_advanced_filter(&proto_query, &node_us));
    assert!(matches_advanced_filter(&proto_query, &node_jp));

    // 4. Region filter
    let mut region_query = ProxyFilterQuery::default();
    region_query.regions = vec![GeoRegion::HongKong];
    assert!(matches_advanced_filter(&region_query, &node_hk));
    assert!(!matches_advanced_filter(&region_query, &node_us));
    assert!(!matches_advanced_filter(&region_query, &node_jp));

    // 5. Latency range filter
    let mut latency_query = ProxyFilterQuery::default();
    latency_query.min_latency_ms = Some(50);
    latency_query.max_latency_ms = Some(100);
    assert!(!matches_advanced_filter(&latency_query, &node_hk));
    assert!(!matches_advanced_filter(&latency_query, &node_us));
    assert!(matches_advanced_filter(&latency_query, &node_jp));

    // 6. Exclusion keyword
    let mut exclude_query = ProxyFilterQuery::default();
    exclude_query.exclude_keywords = vec!["重置".to_owned()];
    assert!(matches_advanced_filter(&exclude_query, &node_hk));
    assert!(!matches_advanced_filter(&exclude_query, &node_us));
    assert!(matches_advanced_filter(&exclude_query, &node_jp));
}
