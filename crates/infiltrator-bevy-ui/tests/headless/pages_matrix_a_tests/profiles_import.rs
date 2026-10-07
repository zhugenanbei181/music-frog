//! Behavior cases for profiles import.
//! test-intent: behavior

use super::*;
use infiltrator_contract::subscription_import::SubscriptionImportChannel;

#[test]
fn test_profiles_import_channels_submit_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();

    set_marker_text::<ImportSubscriptionNameField>(&mut app, "new-sub");
    set_marker_text::<ImportSubscriptionUrlField>(&mut app, "https://example.com/sub");
    set_marker_text::<ImportLocalPathField>(&mut app, "/tmp/local.yaml");

    let url_button = marker_entity::<ImportSubscriptionUrlButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: url_button });
    app.update();
    let local_button = marker_entity::<ImportLocalSubscriptionButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: local_button,
    });
    app.update();
    let clipboard_button = marker_entity::<ImportClipboardSubscriptionButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: clipboard_button,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::ImportSubscription {
                profile_id: "new-sub".to_owned(),
                channel: SubscriptionImportChannel::Url,
                source: "https://example.com/sub".to_owned(),
            },
            UiCommand::ImportSubscription {
                profile_id: "new-sub".to_owned(),
                channel: SubscriptionImportChannel::LocalFile,
                source: "/tmp/local.yaml".to_owned(),
            },
            UiCommand::ImportSubscription {
                profile_id: "new-sub".to_owned(),
                channel: SubscriptionImportChannel::Clipboard,
                source: String::new(),
            },
        ],
        "all three import channels route through the shared command bus"
    );
}
