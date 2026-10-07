//! test-intent: behavior
//! Serialized rule source used by staged-list matrix cases.
use bevy::app::App;
use infiltrator_application::rule_list_application::document_snapshot;
use infiltrator_application::rule_source_identity::rule_workspace;
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};

pub(super) fn seed_rule_draft(app: &mut App) {
    let content = "proxy-groups:\n  - name: PROXY\n    type: select\n    proxies: [DIRECT]\n  - name: Game-Proxy\n    type: select\n    proxies: [DIRECT]\nrules:\n  - DOMAIN-SUFFIX,google.com,PROXY\n  - '# DOMAIN-KEYWORD,tracker,REJECT'\n  - GEOIP,CN,DIRECT\n  - MATCH,DIRECT\n";
    let document =
        document_snapshot(&rule_workspace("rules-fixture.yaml".into(), content).unwrap());
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.pages.rules.data.as_mut().unwrap().document = Some(document);
    snapshot.revision += 1;
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
}
