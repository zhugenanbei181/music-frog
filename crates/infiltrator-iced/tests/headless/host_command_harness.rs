//! Explicit host-capability composition for native host command tests.
use crate::command_harness::test_command_application_for;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_contract::capability::Capability;
use std::iter::empty;

pub fn host_test_command_application(
    capabilities: impl IntoIterator<Item = Capability>,
) -> CommandApplication {
    test_command_application_for(empty(), capabilities)
}
