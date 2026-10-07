//! Behavior cases for options.
//! test-intent: behavior

use super::*;
use infiltrator_domain::mixin::MixinConfig;
use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions};

/// The shared sidecar loader renders the stored mixin back into the editor's
/// YAML buffer, converts the stored filter into the shared draft and publishes
/// the snapshot both surfaces read.
#[tokio::test]
async fn options_load_publishes_the_sidecar_draft_for_both_surfaces() {
    use crate::profile_options_application::ProfileOptionsApplication;

    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    {
        let mut options = store.options.lock().expect("options lock");
        options.insert(
            "main".to_string(),
            ProfileOptions {
                mixin: MixinConfig {
                    mode: Some("global".to_string()),
                    ..Default::default()
                },
                filter: Some(FilterSpec {
                    include_keywords: vec!["香港".to_string()],
                    ..Default::default()
                }),
            },
        );
    }
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let application = ProfileOptionsApplication::new(profiles.clone());

    let snapshot = application.load(None).await.expect("sidecar");
    assert_eq!(snapshot.source.profile, "main");
    assert!(
        snapshot.mixin_yaml.contains("mode: global"),
        "the stored mixin is rendered back into the editor buffer: {}",
        snapshot.mixin_yaml
    );
    assert_eq!(snapshot.filter.include, "香港");
    assert_eq!(
        profiles.editor_observations().1,
        Some(snapshot),
        "the load publishes the same snapshot for the surface projection"
    );
}
