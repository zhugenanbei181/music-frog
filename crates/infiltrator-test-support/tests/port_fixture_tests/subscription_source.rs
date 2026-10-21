//! Behaviour of the isolated subscription-source port fixture.

use infiltrator_domain::subscription::CheckedSubscriptionUrl;
use infiltrator_ports::subscription_source::{ConditionalFetchHeaders, SubscriptionSource};
use infiltrator_test_support::subscription_source_fixtures::{
    CAPTURE_DOCUMENT, CAPTURE_PROFILE, SubscriptionSourceCapture,
};
use lurker_core_ports::error::PortError;

fn capture_url() -> CheckedSubscriptionUrl {
    CheckedSubscriptionUrl::parse("https://subscription.example.com/profile.yaml")
        .expect("valid capture subscription URL")
}

#[tokio::test]
async fn a_failing_source_really_fails() {
    let source = SubscriptionSourceCapture::refusing("capture subscription source is offline");

    let error = source
        .fetch(CAPTURE_PROFILE, &capture_url())
        .await
        .expect_err("the source must fail");

    assert!(matches!(error, PortError::Network(_)));
}

#[tokio::test]
async fn a_failing_source_records_the_profile_the_surface_asked_for() {
    let source = SubscriptionSourceCapture::refusing("capture subscription source is offline");

    let _ = source.fetch(CAPTURE_PROFILE, &capture_url()).await;

    assert_eq!(
        source.requested_profiles(),
        vec![CAPTURE_PROFILE.to_owned()]
    );
}

#[tokio::test]
async fn a_failing_source_also_fails_the_conditional_fetch() {
    let source = SubscriptionSourceCapture::refusing("capture subscription source is offline");

    let error = source
        .fetch_conditional(
            CAPTURE_PROFILE,
            &capture_url(),
            &ConditionalFetchHeaders::default(),
        )
        .await
        .expect_err("the conditional fetch must fail");

    assert!(matches!(error, PortError::Network(_)));
}

#[tokio::test]
async fn an_unhonored_insecure_flag_is_a_typed_unsupported() {
    let source = SubscriptionSourceCapture::serving(CAPTURE_PROFILE, CAPTURE_DOCUMENT);
    let headers = ConditionalFetchHeaders {
        insecure_skip_verify: true,
        ..ConditionalFetchHeaders::default()
    };

    let error = source
        .fetch_conditional(CAPTURE_PROFILE, &capture_url(), &headers)
        .await
        .expect_err("the insecure flag must be refused");

    assert!(matches!(error, PortError::Unsupported { .. }));
}

#[tokio::test]
async fn a_serving_source_publishes_its_document() {
    let source = SubscriptionSourceCapture::serving(CAPTURE_PROFILE, CAPTURE_DOCUMENT);

    let document = source
        .fetch(CAPTURE_PROFILE, &capture_url())
        .await
        .expect("served document");

    assert_eq!(document.content, CAPTURE_DOCUMENT);
}

#[tokio::test]
async fn a_profile_the_source_never_served_is_not_found() {
    let source = SubscriptionSourceCapture::serving(CAPTURE_PROFILE, CAPTURE_DOCUMENT);

    let error = source
        .fetch("another-profile", &capture_url())
        .await
        .expect_err("an unserved profile must not be found");

    assert!(matches!(error, PortError::NotFound(_)));
}
