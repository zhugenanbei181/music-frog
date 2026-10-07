//! Explicit demo/test observations derived from their supplied documents.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::subscription_filter_form::FilterObservation;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::filter_policy_form::filter_spec_from_draft;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profile_source::identify_profile_source;

pub const FIXTURE_DOCUMENT: &str = "mode: rule\n";

pub fn observation(
    profile: &str,
    content: &str,
    filter: SubscriptionFilterDraft,
) -> Result<FilterObservation, Failure> {
    let options = (!filter.is_empty())
        .then(|| {
            filter_spec_from_draft(&filter).and_then(|spec| {
                serde_yaml_ng::to_string(&ProfileOptions {
                    filter: Some(spec),
                    ..Default::default()
                })
                .map_err(Into::into)
            })
        })
        .transpose()
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    Ok(FilterObservation {
        source: identify_profile_source(profile.into(), content, options.as_deref()),
        filter,
    })
}
