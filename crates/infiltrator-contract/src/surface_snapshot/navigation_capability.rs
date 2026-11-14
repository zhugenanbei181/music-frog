use super::*;

pub(crate) fn unobserved_navigation_capability() -> PageData<NavigationCapabilitySnapshot> {
    PageData::unavailable(Failure::new(
        ErrorCode::NotReady,
        "navigation capability owner has not published an observation",
        true,
    ))
}
