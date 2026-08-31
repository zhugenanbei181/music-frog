//! 恢复出厂设置的纯文件系统清理。
//!
//! 本模块只做「删」，不碰内核进程、不碰 [`crate::profiles`] 的全局
//! manager，也不重建 default 配置。调用方（infiltrator-iced 的
//! `Message::FactoryReset`）必须遵守以下执行顺序契约：
//!
//! 1. 内核必须已经停掉（带超时的 runtime shutdown），系统代理与自启
//!    已关闭——否则可能出现删 `versions/` 时内核文件仍被占用；
//! 2. 必须趁 `<home>/settings.toml` 还在时先解析生效的 configs 目录
//!    （settings 的 `configs_dir` 可能指向 iCloud 等云同步目录）并枚举
//!    profile 名清理 OS keyring 中的订阅凭证，**然后**才调用
//!    [`execute`]——settings 一旦删除，云目录就无从得知，残留无法清理；
//! 3. [`execute`] 返回后再调用
//!    [`crate::profiles::reset_profiles_to_default()`] 重建 default 配置：
//!    此时 settings 已删，configs 目录回落 `<home>/configs`，正是出厂态。
//!
//! [`execute`] 的删除语义：
//!
//! * 硬失败组（`settings.toml` / `settings.json` / `config.toml`）删除
//!   失败立即返回 `Err`，中止整个恢复出厂；
//! * 软失败组（`versions/`、`logs/`、崩溃/启动日志、configs 目录）删除
//!   失败只记入 [`ResetReport::warnings`]，不中断其余目标；
//! * 不存在的目标静默跳过并记入 [`ResetReport::skipped`]。

use std::path::{Path, PathBuf};

/// 恢复出厂清理的结果报告。
#[derive(Debug, Default, Clone)]
pub struct ResetReport {
    /// 实际删除成功的路径。
    pub removed: Vec<PathBuf>,
    /// 不存在而静默跳过的目标路径。
    pub skipped: Vec<PathBuf>,
    /// 非致命失败（目录被占用、日志无法删除、configs 目录未解析等）。
    pub warnings: Vec<String>,
}

impl ResetReport {
    /// 成功删除的目标数量（测试与日志用便捷方法）。
    pub fn removed_count(&self) -> usize {
        self.removed.len()
    }
}

/// 按 [`self`] 模块文档的契约执行文件系统清理。
///
/// `home` 是应用主目录；`configs_dir` 是调用方在 settings 删除**前**解析出
/// 的生效 configs 目录（可能被云同步重定向）。`None` 表示解析失败——此时
/// 其余目标照常清理，仅在报告中记录 configs 目录可能残留的 warning。
pub fn execute(home: &Path, configs_dir: Option<&Path>) -> anyhow::Result<ResetReport> {
    if home.as_os_str().is_empty() {
        anyhow::bail!("恢复出厂失败: home 目录为空");
    }
    let mut report = ResetReport::default();

    // 硬失败组：settings 与当前 profile 指针删不掉就必须中止，
    // 半新半旧的状态比完整旧状态更危险。
    for name in ["settings.toml", "settings.json", "config.toml"] {
        remove_file_hard(&home.join(name), &mut report)?;
    }

    // 软失败组：目录/日志删除失败（如被占用）只记 warning，继续其余清理。
    remove_dir_soft(&home.join("versions"), &mut report);
    remove_dir_soft(&home.join("logs"), &mut report);
    remove_file_soft(&home.join("infiltrator_crash.log"), &mut report);
    remove_file_soft(&home.join("startup_critical.log"), &mut report);
    remove_configs_dir(configs_dir, home, &mut report);

    Ok(report)
}

/// settings / config 指针文件：删除失败即 `Err`。
fn remove_file_hard(path: &Path, report: &mut ResetReport) -> anyhow::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => {
            report.removed.push(path.to_path_buf());
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.skipped.push(path.to_path_buf());
            Ok(())
        }
        Err(error) => Err(anyhow::anyhow!("删除 {} 失败: {error}", path.display())),
    }
}

/// 崩溃/启动日志等普通文件：删除失败只记 warning。
fn remove_file_soft(path: &Path, report: &mut ResetReport) {
    match std::fs::remove_file(path) {
        Ok(()) => report.removed.push(path.to_path_buf()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.skipped.push(path.to_path_buf());
        }
        Err(error) => report
            .warnings
            .push(format!("删除 {} 失败: {error}", path.display())),
    }
}

/// 内核版本 / 日志 / configs 目录：删除失败只记 warning。
fn remove_dir_soft(path: &Path, report: &mut ResetReport) {
    if !path.exists() {
        report.skipped.push(path.to_path_buf());
        return;
    }
    match std::fs::remove_dir_all(path) {
        Ok(()) => report.removed.push(path.to_path_buf()),
        Err(error) => report
            .warnings
            .push(format!("删除目录 {} 失败: {error}", path.display())),
    }
}

/// configs 目录带安全护栏的整目录删除（含 cache.db / geoip /
/// options / snapshots 等云同步残留）。
fn remove_configs_dir(configs_dir: Option<&Path>, home: &Path, report: &mut ResetReport) {
    let Some(configs_dir) = configs_dir else {
        report
            .warnings
            .push("configs 目录未解析（settings 读取失败），云同步目录可能残留".to_string());
        return;
    };
    // 护栏：configs 目录异常地覆盖 home（例如 settings 配成了 "." 或上级
    // 目录）时整目录删除会清掉用户主目录，必须跳过。
    if home.starts_with(configs_dir) {
        report.warnings.push(format!(
            "configs 目录 {} 覆盖 home 目录，跳过删除",
            configs_dir.display()
        ));
        return;
    }
    remove_dir_soft(configs_dir, report);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建文件（自动建父目录）。
    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn full_tree_is_removed() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        touch(&home.join("settings.toml"));
        touch(&home.join("settings.json"));
        touch(&home.join("config.toml"));
        touch(&home.join("versions").join("1.19.0").join("mihomo"));
        touch(&home.join("logs").join("mihomo.log"));
        touch(&home.join("infiltrator_crash.log"));
        touch(&home.join("startup_critical.log"));
        // 模拟云同步重定向：configs 不在 home 直下。
        let configs = home.join("cloud").join("profiles");
        touch(&configs.join("cache.db"));
        touch(&configs.join("options").join("options.toml"));
        touch(&configs.join("snapshots").join("snap.json"));

        let report = execute(home, Some(&configs)).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        // 3 个 settings/config 文件 + versions + logs + 2 个日志 + configs。
        assert_eq!(report.removed_count(), 8);

        for path in [
            "settings.toml",
            "settings.json",
            "config.toml",
            "versions/1.19.0/mihomo",
            "logs/mihomo.log",
            "infiltrator_crash.log",
            "startup_critical.log",
        ] {
            assert!(!home.join(path).exists(), "{path} 仍存在");
        }
        assert!(!configs.join("cache.db").exists());
        assert!(!configs.join("options").exists());
        assert!(!configs.join("snapshots").exists());
        assert!(!configs.exists());
    }

    #[test]
    fn missing_targets_are_skipped_silently() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        let configs = home.join("configs");

        let report = execute(home, Some(&configs)).unwrap();
        assert!(report.removed.is_empty());
        assert!(report.warnings.is_empty());
        // 3 个硬失败文件 + versions + logs + 2 个日志 + configs。
        assert_eq!(report.skipped.len(), 8);
        assert!(report.skipped.contains(&configs));
    }

    #[test]
    fn soft_failures_become_warnings_without_aborting() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        touch(&home.join("settings.toml"));
        touch(&home.join("settings.json"));
        touch(&home.join("config.toml"));
        // 目录目标建成普通文件：remove_dir_all 必失败（非 NotFound）。
        touch(&home.join("versions"));
        touch(&home.join("logs"));
        touch(&home.join("configs_dir_target"));
        // 文件目标建成目录：remove_file 必失败（非 NotFound）。
        std::fs::create_dir_all(home.join("infiltrator_crash.log")).unwrap();
        std::fs::create_dir_all(home.join("startup_critical.log")).unwrap();
        let configs = home.join("configs_dir_target");

        let report = execute(home, Some(&configs)).unwrap();
        // 硬失败组照常删除。
        assert_eq!(report.removed_count(), 3);
        assert!(!home.join("settings.toml").exists());
        assert!(!home.join("config.toml").exists());
        // 5 个软目标全部落 warning：versions/logs/configs + 2 个日志。
        assert_eq!(report.warnings.len(), 5, "{:?}", report.warnings);
        assert!(report.skipped.is_empty());
        assert!(home.join("versions").exists());
        assert!(home.join("infiltrator_crash.log").exists());
    }

    #[test]
    fn configs_dir_covering_home_is_not_deleted() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        touch(&home.join("settings.toml"));
        touch(&home.join("important.txt"));

        // configs 目录 = home 本身：必须跳过整目录删除。
        let report = execute(home, Some(home)).unwrap();
        assert!(home.join("important.txt").exists());
        assert!(!home.join("settings.toml").exists());
        assert!(report.warnings.iter().any(|w| w.contains("跳过删除")));
        assert!(!report.removed.iter().any(|p| p == home));
    }

    #[test]
    fn unresolved_configs_dir_is_reported_but_not_fatal() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        touch(&home.join("settings.toml"));
        touch(&home.join("configs").join("cache.db"));

        let report = execute(home, None).unwrap();
        assert!(!home.join("settings.toml").exists());
        // None 不做猜测删除，仅提示可能残留。
        assert!(home.join("configs").exists());
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("configs 目录未解析"))
        );
    }

    #[test]
    fn empty_home_is_rejected() {
        assert!(execute(Path::new(""), None).is_err());
    }
}
