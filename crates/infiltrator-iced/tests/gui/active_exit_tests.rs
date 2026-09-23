use super::*;

#[test]
fn status_text_tracks_shared_liveness_without_guessing() {
    let lang = Lang("en-US");
    let mut snapshot = ActiveExitSnapshot::demo_fixture();
    assert_eq!(status_text(&snapshot, &lang), "Alive");
    snapshot.alive = None;
    assert_eq!(status_text(&snapshot, &lang), "Liveness unknown");
    snapshot.status = ActiveExitStatus::Empty;
    assert_eq!(status_text(&snapshot, &lang), "No active exit");
}

#[test]
fn country_flag_comes_from_the_shared_region_code() {
    let snapshot = ActiveExitSnapshot::demo_fixture();
    assert_eq!(country_flag(&snapshot), "🇭🇰");
    let mut unknown = snapshot;
    unknown.country_code = None;
    assert_eq!(country_flag(&unknown), "🌐");
}
