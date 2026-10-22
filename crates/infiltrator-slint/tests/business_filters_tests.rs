//! The business panels of the filters workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{assert_refused, assert_retried, business_surface};
use infiltrator_slint::surface::business::BusinessId;

/// The page that carries the filters businesses.
const PAGE: &str = "Filters";

#[test]
fn the_profiles_subscription_error_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::ProfilesSubscriptionError);
}

#[test]
fn the_profiles_subscription_error_retry_path() {
    let surface = business_surface(PAGE);

    assert_retried(&surface, BusinessId::ProfilesSubscriptionError);
}
