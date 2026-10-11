//! Snapshot actions use the product command service; storage stays in the host composition.
use crate::command_ingress::CoreCommandIngress;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::InfiltratorError;

pub(crate) async fn execute(
    commands: Option<CoreCommandIngress>,
    intent: CommandIntent,
) -> Result<CommandOutput, InfiltratorError> {
    let commands = commands.ok_or_else(|| {
        InfiltratorError::Config("Snapshot command service is unavailable".into())
    })?;
    commands
        .execute(intent)
        .await
        .into_output()
        .map_err(|failure| InfiltratorError::Config(failure.message))
}
