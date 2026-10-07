//! Memory-port fixtures retain the same optional-document identity semantics.
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profile_source::identify_profile_source;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::ProfileWorkspace;

pub(super) fn workspace(
    profile: &str,
    content: &str,
    options: Option<&ProfileOptions>,
) -> Result<ProfileWorkspace, PortError> {
    let raw = options
        .map(serde_yaml_ng::to_string)
        .transpose()
        .map_err(|error| PortError::Io(error.to_string()))?;
    Ok(ProfileWorkspace {
        write_protection: ProfileWriteProtection::Editable,
        source: identify_profile_source(profile.into(), content, raw.as_deref()),
        content: content.into(),
        options: options.cloned().unwrap_or_default(),
        options_document: raw,
    })
}
