//! The Requests page (最近请求): the request tracker's own readback.
//!
//! FlClash's desktop navigation ships a Requests row whose page carries a
//! live/recent feed switch and a table of the tracker's records. The contract
//! publishes exactly those facts in [`RequestTrackerSnapshot`]: `records`,
//! `active_count`, `total_requests_observed` and `tracking_state`. This page
//! renders them through the shared presentation projection, so it owns no
//! second copy of any business fact.
//!
//! The feed selection is a typed view state with a typed toggle. The
//! reference's live feed also closes live connections and offers five faceted
//! filters (network, rule, MITM, app, proxy chain); the contract publishes
//! neither a live-connection stream for this page nor MITM/rule-facet facts,
//! so the page does not fake them. It states the tracker's own typed tracking
//! status and renders the owner's records instead.

use crate::route::{PageRoot, Route};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, FlexDirection, JustifyContent, Node, Overflow, UiRect, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Button, ScrollArea};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::design_resources;
use infiltrator_contract::request_tracker::RequestStatusFilter;
use infiltrator_contract::request_tracker::{
    RequestHistoryFilter, RequestTrackerSnapshot, RequestTrackingState,
};
use infiltrator_presentation::request_tracker_projection::{
    RequestPageProjection, RequestRowProjection, project_request_page,
};

/// Root marker on the Requests page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct RequestsPageRoot;

/// Which feed the page is showing.
///
/// The reference's `RequestFeed { recent, live }` is a two-member enum, not a
/// boolean: a feed is a typed selection the title names and the user owns.
/// The contract's own status filter is the typed transition between them, so
/// the page never derives "is active" by comparing IDs or status words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RequestFeed {
    /// The tracker's recorded history, newest first.
    #[default]
    Recent,
    /// Only the requests the owner reports as active.
    Live,
}

impl RequestFeed {
    pub const ALL: [Self; 2] = [Self::Recent, Self::Live];

    /// The locale key that names this feed in the page's own title.
    pub fn title_key(self) -> &'static str {
        match self {
            // Chinese product-catalog message ids are the shared copy
            // owner's keys, never bare view literals.
            Self::Recent => LocalizedText::plain("最近请求").key,
            Self::Live => LocalizedText::plain("运行中").key,
        }
    }

    /// The typed toggle: a switch never becomes a boolean the view derives.
    pub const fn toggled(self) -> Self {
        match self {
            Self::Recent => Self::Live,
            Self::Live => Self::Recent,
        }
    }

    /// The owner's typed filter this feed reads.
    pub const fn filter(self) -> RequestHistoryFilter {
        match self {
            Self::Recent => RequestHistoryFilter {
                keyword: None,
                network: None,
                outbound: None,
                status_filter: RequestStatusFilter::All,
            },
            Self::Live => RequestHistoryFilter {
                keyword: None,
                network: None,
                outbound: None,
                status_filter: RequestStatusFilter::ActiveOnly,
            },
        }
    }
}

/// Marker on a feed tab, carrying the typed feed it selects.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RequestFeedTab(pub RequestFeed);

/// Marker on a record row, so a test can prove the published records render.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestRecordRow(pub String);

/// One row of the request table, as the shared projection published it.
fn request_row_scene(row: &RequestRowProjection, palette: &UiPalette) -> impl Scene + use<> {
    let role = match row.status_role {
        "success" => Token::Success,
        "warning" => Token::Warning,
        "danger" => Token::Danger,
        _ => Token::InkDim,
    };
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: rem(space::S4),
            padding: UiRect::all(rem(space::S8)),
            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
        }
        ThemeBackgroundColor({ Token::SurfaceElevated })
        RequestRecordRow({ row.id.clone() })
        Children [
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
            }
            Children [
                Node { flex_direction: FlexDirection::Column }
                Children [
                    Text({ row.host.clone() }) TextRole(design_resources::TextRole::BodyStrong)
                    --
                    Text({ format!("{} · {}", row.time_text, row.network_label) })
                    TextRole(design_resources::TextRole::Caption)
                    ThemeTextColor({ Token::InkDim })
                ]
                --
                Text({ row.status_label.to_owned() })
                TextRole(design_resources::TextRole::Caption)
                ThemeTextColor({ role })
            ]
            --
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
            }
            Children [
                Text({ row.rule_label.clone() }) TextRole(design_resources::TextRole::Caption)
                --
                Text({ row.outbound_label.clone() }) TextRole(design_resources::TextRole::Caption)
            ]
            --
            Text({ format!(
                "{} · {} · {}", row.upload_text, row.download_text, row.duration_text
            ) })
            TextRole(design_resources::TextRole::Caption)
            ThemeTextColor({ Token::InkDim })
        ]
    }
}

/// One feed tab, selected according to the page's typed feed state.
fn feed_tab_scene(feed: RequestFeed, active: RequestFeed, palette: &UiPalette) -> Box<dyn Scene> {
    let _ = palette;
    let selected = feed == active;
    Box::new(bsn! {
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(rem(space::S12), rem(space::S4)),
                border_radius: BorderRadius::MAX,
            }
            ThemeBackgroundColor({ if selected { Token::AccentContainer } else { Token::SurfaceElevated } })
            Button
            RequestFeedTab(feed)
            Children [
                LocalizedText::plain(feed.title_key())
                TextRole(design_resources::TextRole::Caption)
                ThemeTextColor({ if selected { Token::Accent } else { Token::InkDim } })
            ]
    })
}

/// The Requests page: the feed switch, the tracker's own summary, and the
/// published records in a scrollable table.
pub fn requests_page(
    snapshot: &RequestTrackerSnapshot,
    feed: RequestFeed,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let projection = project_request_page(snapshot, &feed.filter());
    let row_scenes: Vec<Box<dyn Scene>> = projection
        .rows
        .iter()
        .map(|row| Box::new(request_row_scene(row, palette)) as Box<dyn Scene>)
        .collect();
    let tabs: Vec<Box<dyn Scene>> = RequestFeed::ALL
        .into_iter()
        .map(|item| feed_tab_scene(item, feed, palette))
        .collect();
    let title_key = feed.title_key();
    // The presentation owner already publishes the localized summary from the
    // snapshot's own counts; the page does not restate those numbers in a
    // second sentence of its own.
    let summary = request_summary_text(&projection);
    let state = snapshot.tracking_state;

    Box::new(bsn! {
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S12),
            }
            PageRoot(Route::Requests)
            RequestsPageRoot
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    LocalizedText::plain(title_key) TextRole(design_resources::TextRole::Heading)
                    --
                    Node { column_gap: rem(space::S4) }
                    Children [ { tabs } ]
                ]
                --
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S4),
                    padding: UiRect::all(rem(space::S8)),
                    border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                }
                ThemeBackgroundColor({ Token::SurfaceElevated })
                Children [
                    Text(summary) TextRole(design_resources::TextRole::Body)
                    --
                    Text({ tracking_state_text(state).to_owned() })
                    TextRole(design_resources::TextRole::Caption)
                    ThemeTextColor({ Token::InkDim })
                ]
                --
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: rem(space::S0),
                    min_height: rem(space::S0),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S4),
                    overflow: Overflow::scroll_y(),
                }
                ScrollArea
                Children [ { row_scenes } ]
            ]
    })
}

fn tracking_state_text(state: RequestTrackingState) -> &'static str {
    match state {
        RequestTrackingState::Active => LocalizedText::plain("运行中").key,
        RequestTrackingState::Paused => LocalizedText::plain("暂停").key,
    }
}

fn request_summary_text(projection: &RequestPageProjection) -> String {
    projection.summary_text.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::request_tracker::{RequestNetwork, RequestRecord, RequestStatus};

    #[test]
    fn the_feed_is_a_typed_two_member_selection() {
        assert_eq!(RequestFeed::ALL.len(), 2);
        assert_eq!(RequestFeed::Recent.toggled(), RequestFeed::Live);
        assert_eq!(RequestFeed::Live.toggled(), RequestFeed::Recent);
    }

    #[test]
    fn the_live_feed_reads_the_owners_active_only_filter() {
        let snapshot = RequestTrackerSnapshot::demo_fixture();
        let live = project_request_page(&snapshot, &RequestFeed::Live.filter());
        assert_eq!(live.filtered_count, 1);
        assert_eq!(live.rows[0].id, "req-2");
        let recent = project_request_page(&snapshot, &RequestFeed::Recent.filter());
        assert_eq!(recent.filtered_count, 3);
    }

    #[test]
    fn the_summary_names_the_snapshot_counts() {
        let snapshot = RequestTrackerSnapshot::demo_fixture();
        let projection = project_request_page(&snapshot, &RequestFeed::Recent.filter());
        let text = request_summary_text(&projection);
        assert!(text.contains("3"));
        assert_eq!(tracking_state_text(RequestTrackingState::Paused), "暂停");
    }

    #[test]
    fn every_published_record_fact_reaches_a_row() {
        let record = RequestRecord::new(
            "req-42",
            1741300000000,
            "api.github.com:443",
            RequestNetwork::Tcp,
            "HongKong-01",
        )
        .with_type_name("HTTPS")
        .with_rule("DomainSuffix", "github.com")
        .with_traffic(1024, 8192, 450)
        .with_status(RequestStatus::Closed);
        let row = RequestRowProjection::from_record(&record);
        assert_eq!(row.host, "api.github.com:443");
        assert_eq!(row.network_label, "TCP · HTTPS");
        assert_eq!(row.rule_label, "DomainSuffix(github.com)");
        assert_eq!(row.outbound_label, "HongKong-01");
        assert_eq!(row.upload_text, "1.00 KB");
        assert_eq!(row.status_label, "已关闭");
    }
}
