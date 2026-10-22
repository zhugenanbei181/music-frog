//! The vocabulary the imported Lumen widgets speak.
//!
//! The imported widgets name a sort dimension, a level, a routing intent, a
//! traffic mode and a theme with typed enums. Those enums are presentation
//! vocabulary, not business state: the shared typed keys own the meaning, and
//! this module is the one adapter between the two. No other place in the
//! surface may translate a widget value on its own, so the widget vocabulary
//! can never grow a second meaning for one control.
//!
//! The shared contract names the log level `LogLevel` as well, so this module
//! imports the shared one and names the widget's own enum `crate::LogLevel`.

use crate::{ConnectionSort, NetworkMode, RouteMode, ThemeMode};
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_domain::connection_view::ConnectionSortKey;
use infiltrator_presentation::log_page_model::LogLevelFilter;
use infiltrator_presentation::overview_page_model::{OverviewNetworkMode, OverviewRoutingMode};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::preferences::OutboundSortOrder;
use lurker_core_contract::routing_intent::RoutingIntent;

/// The shared sort key the widget's sort dimension names.
///
/// The widget offers four dimensions and every one of them names one shared
/// key, so the connection rows are ordered by the shared reduction and not
/// by a second surface-local order.
pub fn connection_sort_key(value: ConnectionSort) -> ConnectionSortKey {
    match value {
        ConnectionSort::Time => ConnectionSortKey::LatestDesc,
        ConnectionSort::Upload => ConnectionSortKey::UploadDesc,
        ConnectionSort::Download => ConnectionSortKey::DownloadDesc,
        ConnectionSort::Host => ConnectionSortKey::HostAsc,
    }
}

/// The shared filter the widget's level names.
///
/// The widget offers one button per level the owner can confirm, plus the
/// "every level" button. The `unknown` level is a real record level, so a
/// page that keeps it names it and no button is selected.
pub fn log_level_filter(value: crate::LogLevel) -> LogLevelFilter {
    match value {
        crate::LogLevel::All => LogLevelFilter::All,
        crate::LogLevel::Debug => LogLevelFilter::Level(LogLevel::Debug),
        crate::LogLevel::Info => LogLevelFilter::Level(LogLevel::Info),
        crate::LogLevel::Warning => LogLevelFilter::Level(LogLevel::Warn),
        crate::LogLevel::Error => LogLevelFilter::Level(LogLevel::Error),
        crate::LogLevel::Unknown => LogLevelFilter::Level(LogLevel::Unknown),
    }
}

/// The widget's level for the shared level the owner confirmed.
///
/// `None` is "every level". The widget has no button for the `unknown`
/// record level, so the widget names it and no level button is selected; the
/// surface never claims a level the owner did not confirm.
pub fn log_level_value(level: Option<LogLevel>) -> crate::LogLevel {
    match level {
        None => crate::LogLevel::All,
        Some(LogLevel::Debug) => crate::LogLevel::Debug,
        Some(LogLevel::Info) => crate::LogLevel::Info,
        Some(LogLevel::Warn) => crate::LogLevel::Warning,
        Some(LogLevel::Error) => crate::LogLevel::Error,
        Some(LogLevel::Unknown) => crate::LogLevel::Unknown,
    }
}

/// The shared routing intent the widget's routing mode names.
///
/// The widget offers three modes and every one of them names one shared
/// intent, so the owner is asked for the intent and never for a word.
pub fn route_intent(value: RouteMode) -> Option<RoutingIntent> {
    match value {
        RouteMode::Rule => Some(RoutingIntent::FollowRules),
        RouteMode::Global => Some(RoutingIntent::RouteAllThroughProxy),
        RouteMode::Direct => Some(RoutingIntent::BypassProxy),
        RouteMode::Unknown => None,
    }
}

/// The routing mode the owner published, as the widget's own vocabulary.
///
/// The owner publishes an unknown mode before the routing resolved, so the
/// widget vocabulary keeps an `unknown` variant instead of pretending a mode.
pub fn route_mode(value: OverviewRoutingMode) -> crate::RouteMode {
    match value {
        OverviewRoutingMode::Rule => crate::RouteMode::Rule,
        OverviewRoutingMode::Global => crate::RouteMode::Global,
        OverviewRoutingMode::Direct => crate::RouteMode::Direct,
        OverviewRoutingMode::Unknown => crate::RouteMode::Unknown,
    }
}

/// The traffic mode the owner published, as the widget's own vocabulary.
pub fn network_mode(value: OverviewNetworkMode) -> crate::NetworkMode {
    match value {
        OverviewNetworkMode::Vpn => crate::NetworkMode::Vpn,
        OverviewNetworkMode::Tun => crate::NetworkMode::Tun,
        OverviewNetworkMode::SystemProxy => crate::NetworkMode::SystemProxy,
        OverviewNetworkMode::CoreOnly => crate::NetworkMode::CoreOnly,
        OverviewNetworkMode::Unknown => crate::NetworkMode::Unknown,
    }
}

/// The lifecycle the owner published, as the widget's own vocabulary.
pub fn runtime_status(value: CoreLifecycle) -> crate::RuntimeStatus {
    match value {
        CoreLifecycle::Stopped => crate::RuntimeStatus::Stopped,
        CoreLifecycle::Starting => crate::RuntimeStatus::Starting,
        CoreLifecycle::Ready | CoreLifecycle::Running => crate::RuntimeStatus::Running,
        CoreLifecycle::Stopping => crate::RuntimeStatus::Stopping,
        CoreLifecycle::Failed => crate::RuntimeStatus::Failed,
    }
}

/// The theme the owner stored, as the widget's own vocabulary.
pub fn theme_mode(value: &str) -> crate::ThemeMode {
    match value {
        "light" => crate::ThemeMode::Light,
        "dark" => crate::ThemeMode::Dark,
        "forest" => crate::ThemeMode::Forest,
        "amoled" => crate::ThemeMode::Amoled,
        _ => crate::ThemeMode::System,
    }
}

/// The routing mode the action layer names.
///
/// A mode the widget does not offer names no action at all, so the caller can
/// refuse the gesture instead of routing the traffic the wrong way.
pub fn route_mode_action_name(value: RouteMode) -> Option<&'static str> {
    match value {
        RouteMode::Rule => Some("Rule"),
        RouteMode::Global => Some("Global"),
        RouteMode::Direct => Some("Direct"),
        RouteMode::Unknown => None,
    }
}

/// The traffic mode the action layer names.
pub fn network_mode_action_name(value: NetworkMode) -> Option<&'static str> {
    match value {
        NetworkMode::Vpn => Some("VPN"),
        NetworkMode::Tun => Some("TUN"),
        NetworkMode::SystemProxy => Some("System Proxy"),
        NetworkMode::CoreOnly => Some("Core only"),
        NetworkMode::Unknown => None,
    }
}

/// The routing mode the action layer names, as the widget's own vocabulary.
pub fn route_mode_from_action_name(value: &str) -> crate::RouteMode {
    match value {
        "Global" => crate::RouteMode::Global,
        "Direct" => crate::RouteMode::Direct,
        "Rule" => crate::RouteMode::Rule,
        _ => crate::RouteMode::Unknown,
    }
}

/// The traffic mode the action layer names, as the widget's own vocabulary.
pub fn network_mode_from_action_name(value: &str) -> crate::NetworkMode {
    match value {
        "VPN" => crate::NetworkMode::Vpn,
        "TUN" => crate::NetworkMode::Tun,
        "System Proxy" => crate::NetworkMode::SystemProxy,
        "Core only" => crate::NetworkMode::CoreOnly,
        _ => crate::NetworkMode::Unknown,
    }
}

/// The theme the owner stored, as the widget's own vocabulary.
///
/// A name the owner never stored names no theme at all, so the caller can
/// refuse the gesture instead of silently switching the user to another skin.
pub fn theme_mode_from_setting_name(value: &str) -> Option<crate::ThemeMode> {
    match value {
        "system" => Some(crate::ThemeMode::System),
        "light" => Some(crate::ThemeMode::Light),
        "dark" => Some(crate::ThemeMode::Dark),
        "forest" => Some(crate::ThemeMode::Forest),
        "amoled" => Some(crate::ThemeMode::Amoled),
        _ => None,
    }
}

/// The lifecycle the widget's runtime status names, as the projected label.
pub fn runtime_status_text(value: crate::RuntimeStatus) -> &'static str {
    match value {
        crate::RuntimeStatus::Stopped => "Stopped",
        crate::RuntimeStatus::Starting => "Starting",
        crate::RuntimeStatus::Running => "Running",
        crate::RuntimeStatus::Stopping => "Stopping",
        crate::RuntimeStatus::Failed => "Failed",
    }
}

/// The routing mode the widget's routing mode names, as the projected label.
pub fn route_mode_text(value: crate::RouteMode) -> &'static str {
    match value {
        crate::RouteMode::Rule => "Rule",
        crate::RouteMode::Global => "Global",
        crate::RouteMode::Direct => "Direct",
        crate::RouteMode::Unknown => "Unknown",
    }
}

/// The traffic mode the widget's traffic mode names, as the projected label.
pub fn network_mode_text(value: crate::NetworkMode) -> &'static str {
    match value {
        crate::NetworkMode::Vpn => "VPN",
        crate::NetworkMode::Tun => "TUN",
        crate::NetworkMode::SystemProxy => "System Proxy",
        crate::NetworkMode::CoreOnly => "Core only",
        crate::NetworkMode::Unknown => "Unknown",
    }
}

/// The sort dimension the action layer names, as the widget's own vocabulary.
pub fn connection_sort_from_action_name(value: &str) -> crate::ConnectionSort {
    match value {
        "upload" => crate::ConnectionSort::Upload,
        "download" => crate::ConnectionSort::Download,
        "host" => crate::ConnectionSort::Host,
        "time" => crate::ConnectionSort::Time,
        _ => crate::ConnectionSort::Time,
    }
}

/// The log level the action layer names, as the widget's own vocabulary.
pub fn log_level_from_action_name(value: &str) -> crate::LogLevel {
    match value {
        "all" => crate::LogLevel::All,
        "debug" => crate::LogLevel::Debug,
        "info" => crate::LogLevel::Info,
        "warn" | "warning" => crate::LogLevel::Warning,
        "error" => crate::LogLevel::Error,
        _ => crate::LogLevel::Unknown,
    }
}

/// The log level the action layer names.
pub fn log_level_action_name(value: crate::LogLevel) -> &'static str {
    match value {
        crate::LogLevel::All => "all",
        crate::LogLevel::Debug => "debug",
        crate::LogLevel::Info => "info",
        crate::LogLevel::Warning => "warn",
        crate::LogLevel::Error => "error",
        crate::LogLevel::Unknown => "unknown",
    }
}

/// The shared traffic mode the widget's traffic mode names.
///
/// The widget offers the three capture modes the desktop host applies. The
/// owner reads the same three names, so the adapter is the one place the two
/// vocabularies meet.
pub fn network_mode_setting(value: NetworkMode) -> &'static str {
    match value {
        NetworkMode::SystemProxy => "system_proxy",
        NetworkMode::Tun => "tun",
        NetworkMode::CoreOnly => "core_only",
        NetworkMode::Vpn => "vpn",
        NetworkMode::Unknown => "unknown",
    }
}

/// The shared theme setting the widget's theme mode names.
///
/// The widget offers the five themes the owner stores, so the adapter hands
/// the owner the very name it published.
pub fn theme_mode_setting(value: ThemeMode) -> &'static str {
    match value {
        ThemeMode::System => "system",
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
        ThemeMode::Forest => "forest",
        ThemeMode::Amoled => "amoled",
    }
}

/// The shared sort order the widget's proxy sort vocabulary names.
///
/// The imported sort menu offers four dimensions and each of them names one
/// shared sort order, so the proxy rows are ordered by the shared reduction
/// and not by a second surface-local order. The widget's menu positions are
/// its own vocabulary — they are not the shared model's control positions —
/// so the adapter maps a dimension by its name.
///
/// A value the widget does not offer names no order at all, so the caller
/// can refuse the gesture instead of silently applying an order the user
/// never picked.
pub fn proxy_sort_order(value: &str) -> Option<OutboundSortOrder> {
    match value {
        "latency_asc" => Some(OutboundSortOrder::LatencyAscending),
        "latency_desc" => Some(OutboundSortOrder::LatencyDescending),
        "name_asc" => Some(OutboundSortOrder::NameAscending),
        "name_desc" => Some(OutboundSortOrder::NameDescending),
        _ => None,
    }
}

/// The widget's proxy sort vocabulary value for one shared sort order.
///
/// Every shared sort order the proxies page offers has exactly one widget
/// value, so the widget and the shared model can never disagree about which
/// dimension the user picked.
pub fn proxy_sort_value(order: OutboundSortOrder) -> &'static str {
    match order {
        OutboundSortOrder::LatencyAscending => "latency_asc",
        OutboundSortOrder::LatencyDescending => "latency_desc",
        OutboundSortOrder::NameAscending => "name_asc",
        OutboundSortOrder::NameDescending => "name_desc",
    }
}

/// The shared Overview card the widget's card id names.
///
/// The Overview card list names every card with the shared card's own id, so
/// the adapter maps the id back to the shared kind. An id the widget does not
/// offer names no card, so the caller can refuse the gesture instead of
/// moving a card the user never picked.
pub fn overview_card_kind(value: &str) -> Option<OverviewCardKind> {
    OverviewCardKind::DEFAULT_ORDER
        .into_iter()
        .find(|kind| kind.as_str() == value)
}

#[cfg(test)]
mod tests {
    use super::{
        connection_sort_key, log_level_filter, log_level_value, network_mode_setting,
        overview_card_kind, proxy_sort_order, proxy_sort_value, route_intent, theme_mode_setting,
    };
    use crate::{ConnectionSort, NetworkMode, RouteMode, ThemeMode};
    use infiltrator_contract::logs::LogLevel;
    use infiltrator_contract::overview_layout::OverviewCardKind;
    use infiltrator_domain::connection_view::ConnectionSortKey;
    use infiltrator_presentation::log_page_model::LogLevelFilter;
    use infiltrator_presentation::proxies_page_model::PROXY_SORT_OPTIONS;
    use lurker_core_contract::routing_intent::RoutingIntent;

    #[test]
    fn the_widget_vocabulary_names_every_shared_proxy_sort_order() {
        for order in PROXY_SORT_OPTIONS {
            let value = proxy_sort_value(order);
            assert_eq!(
                proxy_sort_order(value),
                Some(order),
                "the widget value {value:?} must name the shared order {order:?}"
            );
        }
    }

    #[test]
    fn a_widget_value_the_menu_does_not_offer_names_no_order() {
        assert_eq!(proxy_sort_order("random"), None);
        assert_eq!(proxy_sort_order(""), None);
    }

    #[test]
    fn every_shared_overview_card_has_exactly_one_widget_id() {
        for kind in OverviewCardKind::DEFAULT_ORDER {
            assert_eq!(overview_card_kind(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn an_id_the_card_list_does_not_offer_names_no_card() {
        assert_eq!(overview_card_kind("not-a-card"), None);
        assert_eq!(overview_card_kind(""), None);
    }

    #[test]
    fn every_connection_sort_dimension_names_one_shared_key() {
        assert_eq!(
            connection_sort_key(ConnectionSort::Time),
            ConnectionSortKey::LatestDesc
        );
        assert_eq!(
            connection_sort_key(ConnectionSort::Upload),
            ConnectionSortKey::UploadDesc
        );
        assert_eq!(
            connection_sort_key(ConnectionSort::Download),
            ConnectionSortKey::DownloadDesc
        );
        assert_eq!(
            connection_sort_key(ConnectionSort::Host),
            ConnectionSortKey::HostAsc
        );
    }

    #[test]
    fn every_routing_mode_names_one_shared_intent() {
        assert_eq!(
            route_intent(RouteMode::Rule),
            Some(RoutingIntent::FollowRules)
        );
        assert_eq!(
            route_intent(RouteMode::Global),
            Some(RoutingIntent::RouteAllThroughProxy)
        );
        assert_eq!(
            route_intent(RouteMode::Direct),
            Some(RoutingIntent::BypassProxy)
        );
        assert_eq!(route_intent(RouteMode::Unknown), None);
    }

    #[test]
    fn every_log_level_names_one_shared_filter() {
        assert_eq!(log_level_filter(crate::LogLevel::All), LogLevelFilter::All);
        assert_eq!(
            log_level_filter(crate::LogLevel::Debug),
            LogLevelFilter::Level(LogLevel::Debug)
        );
        assert_eq!(
            log_level_filter(crate::LogLevel::Warning),
            LogLevelFilter::Level(LogLevel::Warn)
        );
        assert_eq!(
            log_level_filter(crate::LogLevel::Unknown),
            LogLevelFilter::Level(LogLevel::Unknown)
        );
    }

    #[test]
    fn the_shared_level_reads_back_as_the_widget_level() {
        assert_eq!(log_level_value(None), crate::LogLevel::All);
        assert_eq!(log_level_value(Some(LogLevel::Info)), crate::LogLevel::Info);
        assert_eq!(
            log_level_value(Some(LogLevel::Unknown)),
            crate::LogLevel::Unknown
        );
    }

    #[test]
    fn every_traffic_mode_and_theme_names_the_shared_setting() {
        assert_eq!(
            network_mode_setting(NetworkMode::SystemProxy),
            "system_proxy"
        );
        assert_eq!(network_mode_setting(NetworkMode::Tun), "tun");
        assert_eq!(network_mode_setting(NetworkMode::CoreOnly), "core_only");
        assert_eq!(network_mode_setting(NetworkMode::Vpn), "vpn");
        assert_eq!(theme_mode_setting(ThemeMode::System), "system");
        assert_eq!(theme_mode_setting(ThemeMode::Amoled), "amoled");
    }
}
