use super::*;

#[cfg(test)]
pub(crate) fn test_request_at_current_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> CoreCommandRequest {
    CoreCommandRequest {
        scope: application
            .snapshot()
            .scope()
            .expect("Core application test fixture must expose its owner identity"),
        intent,
    }
}

#[cfg(test)]
pub(crate) async fn test_execute_at_current_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> CoreCommandOutcome {
    application
        .execute_scoped(test_request_at_current_scope(application, intent))
        .await
}

#[cfg(test)]
pub(crate) async fn test_execute_result_at_current_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> CommandResult {
    test_execute_at_current_scope(application, intent)
        .await
        .result
}

#[cfg(test)]
pub(crate) fn test_dispatch_at_current_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> (RequestId, Receiver<CoreCommandOutcome>) {
    application.dispatch_scoped_tracked(test_request_at_current_scope(application, intent))
}
