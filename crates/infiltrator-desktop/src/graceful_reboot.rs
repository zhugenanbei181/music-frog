use serde::{Serialize, Deserialize};
use anyhow::{anyhow, Result};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RebootReason {
    UserRequested,
    CoreFatalCrash,
    PrivilegeElevation,
    ProfileMigrated,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RebootPlan {
    pub reason: RebootReason,
    pub binary_path: String,
    pub extra_args: Vec<String>,
    pub preserve_environment: bool,
    pub delay_ms: u64,
}

pub struct GracefulRebootCoordinator;

impl GracefulRebootCoordinator {
    pub fn prepare_reboot_plan(
        reason: RebootReason,
        binary_path: &str,
        extra_args: Vec<String>,
    ) -> Result<RebootPlan> {
        let plan = RebootPlan {
            reason,
            binary_path: binary_path.to_string(),
            extra_args,
            preserve_environment: true,
            delay_ms: 0,
        };

        Self::validate_plan(&plan)?;
        Ok(plan)
    }

    pub fn format_command_line(plan: &RebootPlan) -> String {
        let mut cmd = format!("\"{}\"", plan.binary_path);
        for arg in &plan.extra_args {
            cmd.push_str(&format!(" \"{}\"", arg));
        }
        cmd
    }

    pub fn validate_plan(plan: &RebootPlan) -> Result<()> {
        if plan.binary_path.trim().is_empty() {
            return Err(anyhow!("Binary path cannot be empty"));
        }
        
        for arg in &plan.extra_args {
            if arg.contains('\0') {
                return Err(anyhow!("Argument contains null byte"));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_plan() {
        let args = vec!["--test".to_string(), "abc".to_string()];
        let plan = GracefulRebootCoordinator::prepare_reboot_plan(
            RebootReason::UserRequested,
            "/usr/bin/test",
            args.clone(),
        ).unwrap();

        assert_eq!(plan.reason, RebootReason::UserRequested);
        assert_eq!(plan.binary_path, "/usr/bin/test");
        assert_eq!(plan.extra_args, args);
        assert!(plan.preserve_environment);
        assert_eq!(plan.delay_ms, 0);
    }

    #[test]
    fn test_prepare_plan_all_reasons() {
        let reasons = vec![
            RebootReason::UserRequested,
            RebootReason::CoreFatalCrash,
            RebootReason::PrivilegeElevation,
            RebootReason::ProfileMigrated,
        ];

        for reason in reasons {
            let plan = GracefulRebootCoordinator::prepare_reboot_plan(
                reason.clone(),
                "/bin/sh",
                vec![],
            ).unwrap();
            assert_eq!(plan.reason, reason);
        }
    }

    #[test]
    fn test_format_command_line() {
        let plan = RebootPlan {
            reason: RebootReason::UserRequested,
            binary_path: "/bin/my_app".to_string(),
            extra_args: vec!["--flag".to_string(), "val".to_string()],
            preserve_environment: true,
            delay_ms: 0,
        };

        let cmd = GracefulRebootCoordinator::format_command_line(&plan);
        assert_eq!(cmd, "\"/bin/my_app\" \"--flag\" \"val\"");
    }

    #[test]
    fn test_format_command_line_no_args() {
        let plan = RebootPlan {
            reason: RebootReason::UserRequested,
            binary_path: "/bin/my_app".to_string(),
            extra_args: vec![],
            preserve_environment: true,
            delay_ms: 0,
        };

        let cmd = GracefulRebootCoordinator::format_command_line(&plan);
        assert_eq!(cmd, "\"/bin/my_app\"");
    }

    #[test]
    fn test_validate_plan_empty_binary() {
        let plan = RebootPlan {
            reason: RebootReason::UserRequested,
            binary_path: "".to_string(),
            extra_args: vec![],
            preserve_environment: true,
            delay_ms: 0,
        };

        let result = GracefulRebootCoordinator::validate_plan(&plan);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Binary path cannot be empty");
    }

    #[test]
    fn test_validate_plan_null_byte_arg() {
        let plan = RebootPlan {
            reason: RebootReason::UserRequested,
            binary_path: "/bin/test".to_string(),
            extra_args: vec!["hello\0world".to_string()],
            preserve_environment: true,
            delay_ms: 0,
        };

        let result = GracefulRebootCoordinator::validate_plan(&plan);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Argument contains null byte");
    }
}
