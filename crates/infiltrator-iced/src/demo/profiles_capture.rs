//! The profiles page performs its own subscription refresh and aggregation
//! preview gestures.
//!
//! Both gestures run the *shared* application against an isolated adapter, so
//! the surface renders a real typed answer instead of a fabricated one.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::profile_aggregation_application::ProfileAggregationApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_contract::parity::FeatureId;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_test_support::filter_capture_store::{FILTER_PROFILE, FilterCaptureStore};
use infiltrator_test_support::profile_store_fixtures::{CAPTURE_PROFILE, ProfileStoreCapture};
use infiltrator_test_support::subscription_source_fixtures::SubscriptionSourceCapture;
use lurker_core_contract::error::InfiltratorError;
use std::sync::Arc;

/// The subscription URL the capture's refusing source answers for.
const CAPTURE_SUBSCRIPTION_URL: &str = "https://sub.example.com/capture/profile.yaml";

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::ProfilesSubscriptionError => activate_subscription_failure(state),
        FeatureId::ProfilesAggregatorPreview => activate_aggregation_preview(state),
        _ => Task::none(),
    }
}

/// The subscription refresh really fails: the shared application asks the
/// capture's refusing source for the document and the owner answers with its
/// typed network failure, which the surface reports.
fn activate_subscription_failure(state: &mut AppState) -> Task<Message> {
    // The demo reads the profile the capture store publishes, so the refresh
    // really asks the source for that profile.
    state.profile.subscription_profile_name = CAPTURE_PROFILE.to_string();
    state.profile.subscription_url = CAPTURE_SUBSCRIPTION_URL.to_string();
    Task::perform(
        async move {
            let store = Arc::new(ProfileStoreCapture::default());
            let mut metadata = ProfileMetadata::default();
            metadata.subscription_url = Some(CAPTURE_SUBSCRIPTION_URL.to_string());
            let _ = store
                .update_profile_metadata(CAPTURE_PROFILE, &metadata)
                .await;
            let application = ProfileApplication::new(store);
            let source =
                SubscriptionSourceCapture::refusing("the capture subscription source is offline");
            match application
                .update_subscription_conditional(&source, CAPTURE_PROFILE)
                .await
            {
                Ok(report) => Message::SubscriptionUpdatedNow(Ok(report)),
                Err(failure) => {
                    Message::SubscriptionUpdatedNow(Err(InfiltratorError::Config(failure.message)))
                }
            }
        },
        |message| message,
    )
}

/// The aggregation preview really runs: the shared aggregation application
/// reads the capture store's real documents and publishes its report, which
/// the aggregator modal renders.
fn activate_aggregation_preview(state: &mut AppState) -> Task<Message> {
    // The store publishes a real profile document with real proxy nodes, so
    // the shared aggregator has something to aggregate.
    let store = Arc::new(FilterCaptureStore::default());
    // The wizard edits the capture store's own profile, so the modal lists
    // exactly the source the shared application reads.
    state.profile.profiles = vec![ProfileInfo {
        name: FILTER_PROFILE.to_string(),
        path: format!("/isolated-capture/{FILTER_PROFILE}.yaml"),
        active: true,
        ..Default::default()
    }];
    let _ = state.update(Message::OpenAggregatorModal);
    let _ = state.update(Message::UpdateAggregatorName(
        "Aggregated-Capture".to_string(),
    ));
    let Ok(draft) = state.aggregator_draft() else {
        return Task::none();
    };
    Task::perform(
        async move {
            let application = ProfileAggregationApplication::new(profiles_for_mihomo(store));
            match application.preview(&draft).await {
                Ok(report) => Message::AggregationPreviewFinished(Ok(report)),
                Err(failure) => Message::AggregationPreviewFinished(Err(InfiltratorError::Config(
                    failure.message,
                ))),
            }
        },
        |message| message,
    )
}
