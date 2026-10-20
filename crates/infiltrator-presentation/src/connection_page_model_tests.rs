//! test-intent: behavior
use crate::connection_page_model::{
    CONNECTION_SORT_OPTIONS, ConnectionsPageModel, sort_option_at, sort_option_index,
};
use infiltrator_contract::connection::ConnectionRateObservation;
use infiltrator_domain::connection_rate::{ConnectionRate, ConnectionRates};
use infiltrator_domain::connection_view::{
    ConnectionGroupingMode, ConnectionSortKey, ConnectionView,
};

#[derive(Clone, Debug)]
struct Row {
    id: String,
    host: String,
    process: String,
    upload: u64,
    download: u64,
    start: String,
}

impl ConnectionView for Row {
    fn view_id(&self) -> &str {
        &self.id
    }
    fn view_host(&self) -> &str {
        &self.host
    }
    fn view_process_path(&self) -> &str {
        &self.process
    }
    fn view_upload_total(&self) -> u64 {
        self.upload
    }
    fn view_download_total(&self) -> u64 {
        self.download
    }
    fn view_start(&self) -> &str {
        &self.start
    }
}

fn row(id: &str, host: &str, process: &str, upload: u64, download: u64) -> Row {
    Row {
        id: id.to_string(),
        host: host.to_string(),
        process: process.to_string(),
        upload,
        download,
        start: "2026-10-20T00:00:00Z".to_string(),
    }
}

fn rows() -> Vec<Row> {
    vec![
        row("1", "b.example", "/usr/bin/git", 100, 500),
        row("2", "a.example", "/usr/bin/curl", 900, 200),
    ]
}

#[test]
fn the_sort_control_has_one_shared_order_and_one_key_per_position() {
    assert_eq!(CONNECTION_SORT_OPTIONS.len(), 6);
    for (index, key) in CONNECTION_SORT_OPTIONS.iter().enumerate() {
        assert_eq!(sort_option_index(*key), index);
        assert_eq!(sort_option_at(index), *key);
    }

    // A position beyond the list is not a second meaning: it is the default.
    assert_eq!(sort_option_at(99), ConnectionSortKey::default());
    assert_eq!(sort_option_index(ConnectionSortKey::default()), 0);
}

#[test]
fn the_page_model_owns_the_sort_key_and_the_row_order() {
    let mut model = ConnectionsPageModel::<Row>::default();
    assert_eq!(model.sort_key(), ConnectionSortKey::DownloadDesc);
    assert_eq!(model.sort_index(), 0);

    model.observe(&rows());
    assert_eq!(model.source_count(), 2);
    assert_eq!(
        model
            .visible_rows()
            .iter()
            .map(|row| row.view_id())
            .collect::<Vec<_>>(),
        ["1", "2"]
    );

    model.select_sort(ConnectionSortKey::HostAsc);
    assert_eq!(model.sort_key(), ConnectionSortKey::HostAsc);
    // The host order is the last position of the shared sort list.
    assert_eq!(model.sort_index(), 5);

    let mut ordered: Vec<Row> = model.visible_rows().into_iter().cloned().collect();
    model.sort_rows(&mut ordered);
    assert_eq!(
        ordered
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["2", "1"]
    );

    // A control position selects exactly one shared key.
    model.select_sort_at(1);
    assert_eq!(model.sort_key(), ConnectionSortKey::UploadDesc);
    assert_eq!(model.sort_index(), 1);
}

#[test]
fn the_page_model_owns_the_grouping_and_the_query_fold() {
    let mut model = ConnectionsPageModel::<Row>::default();
    model.observe(&rows());

    assert_eq!(model.mode(), ConnectionGroupingMode::Flat);
    assert_eq!(model.query(), "");
    assert_eq!(model.matched_count(), 2);

    model.select(ConnectionGroupingMode::ByProcess);
    assert_eq!(model.mode(), ConnectionGroupingMode::ByProcess);
    assert_eq!(model.rows().len(), 2);

    model.edit_query("curl");
    assert_eq!(model.query(), "curl");
    assert_eq!(model.matched_count(), 1);
    assert_eq!(model.source_count(), 2);
    assert_eq!(
        model
            .visible_rows()
            .iter()
            .map(|row| row.view_id())
            .collect::<Vec<_>>(),
        ["2"]
    );
    assert!(model.search_row("2").is_some_and(|row| row.visible));
    assert!(model.search_row("1").is_none_or(|row| !row.visible));

    model.clear_observation();
    assert!(!model.source_current());
    assert_eq!(model.source_count(), 0);
}

#[test]
fn the_rate_keys_rank_on_the_shared_rate_book() {
    let rates = ConnectionRates::from_pairs([
        (
            "1".to_string(),
            ConnectionRate {
                observation: ConnectionRateObservation::Measured,
                upload_bps: 0.0,
                download_bps: 9_000.0,
            },
        ),
        (
            "2".to_string(),
            ConnectionRate {
                observation: ConnectionRateObservation::Measured,
                upload_bps: 5_000.0,
                download_bps: 0.0,
            },
        ),
    ]);

    let mut model = ConnectionsPageModel::<Row>::default();
    model.observe(&rows());

    model.select_sort(ConnectionSortKey::DownloadRateDesc);
    let rated = model.ordered_rated_rows(&rates);
    assert_eq!(
        rated
            .iter()
            .map(|row| row.row.view_id())
            .collect::<Vec<_>>(),
        ["1", "2"]
    );

    model.select_sort(ConnectionSortKey::UploadRateDesc);
    let rated = model.ordered_rated_rows(&rates);
    assert_eq!(
        rated
            .iter()
            .map(|row| row.row.view_id())
            .collect::<Vec<_>>(),
        ["2", "1"]
    );
}
