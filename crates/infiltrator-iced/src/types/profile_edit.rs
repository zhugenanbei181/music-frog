//! Correlated TEA replies from the product's shared editor commands.
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document::ProfileDocumentSnapshot;
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_presentation::profile_edit_session::ProfileEditPending;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ProfileDocumentReadReply {
    pub ticket: u64,
    pub path: PathBuf,
    pub result: Result<ProfileDocumentSnapshot, Failure>,
}
#[derive(Clone, Debug)]
pub struct ProfileOptionsReadReply {
    pub ticket: u64,
    pub profile: String,
    pub result: Result<ProfileOptionsSnapshot, Failure>,
}
#[derive(Clone, Debug)]
pub struct ProfileEditReply {
    pub pending: ProfileEditPending,
    pub result: Result<CommandOutput, Failure>,
}
