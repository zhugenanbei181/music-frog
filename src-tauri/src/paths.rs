use std::{env, path::PathBuf};

use anyhow::anyhow;
use tauri::{path::BaseDirectory, AppHandle, Manager};

pub(crate) fn resolve_main_dir(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let dev_main = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../webui/mihomo-manager-ui");

    let main_dir = if let Ok(custom) = env::var("METACUBEXD_STATIC_DIR") {
        let path = PathBuf::from(custom);
        if path.exists() {
            path
        } else {
            dev_main.clone()
        }
    } else if dev_main.exists() {
        dev_main
    } else {
        app.path().resolve("bin/mihomo-manager-ui", BaseDirectory::Resource)?
    };

    if !main_dir.exists() {
        return Err(anyhow!(
            "未找到 Mihomo Manager UI 静态资源，请将内容放到 webui/mihomo-manager-ui/ 目录"
        ));
    }

    Ok(main_dir)
}

pub(crate) fn resolve_admin_dir(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let dev_admin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../webui/config-manager-ui");
    let dev_admin_dist = dev_admin.join("dist");

    let admin_dir = if let Ok(custom) = env::var("METACUBEXD_ADMIN_DIR") {
        let path = PathBuf::from(custom);
        if path.exists() {
            path
        } else {
            dev_admin_dist.clone()
        }
    } else if dev_admin_dist.exists() {
        dev_admin_dist
    } else if dev_admin.exists() {
        dev_admin
    } else {
        app.path()
            .resolve("bin/config-manager-ui", BaseDirectory::Resource)?
    };

    if !admin_dir.exists() {
        return Err(anyhow!(
            "未找到配置管理静态资源，请构建 webui/config-manager-ui/ 目录"
        ));
    }

    Ok(admin_dir)
}

pub(crate) fn app_data_dir(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let resolver = app.path();
    resolver
        .app_local_data_dir()
        .or_else(|_| resolver.app_data_dir())
        .map_err(|e: tauri::Error| anyhow!(e.to_string()))
}

pub(crate) fn bundled_core_candidates(app: &AppHandle) -> Vec<PathBuf> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Vec::new()
    }

    #[cfg(target_os = "windows")]
    {
        let mut candidates = Vec::new();
        let project_resource = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/mihomo.exe");
        if project_resource.exists() {
            candidates.push(project_resource);
        }
        let project_resource_legacy =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/mihomo-windows-amd64-v3.exe");
        if project_resource_legacy.exists() {
            candidates.push(project_resource_legacy);
        }

        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let install_dir = exe_dir.join("bin").join("mihomo");
                let install_path = install_dir.join("mihomo.exe");
                if install_path.exists() {
                    candidates.push(install_path);
                }
                let install_path_legacy = install_dir.join("mihomo-windows-amd64-v3.exe");
                if install_path_legacy.exists() {
                    candidates.push(install_path_legacy);
                }
            }
        }

        if let Ok(resource_dir) = app.path().resource_dir() {
            let resource_dir = resource_dir.join("bin").join("mihomo");
            let resource_path = resource_dir.join("mihomo.exe");
            if resource_path.exists() {
                candidates.push(resource_path);
            }
            let resource_path_legacy = resource_dir.join("mihomo-windows-amd64-v3.exe");
            if resource_path_legacy.exists() {
                candidates.push(resource_path_legacy);
            }
        }

        if let Ok(resource_path) = app
            .path()
            .resolve("bin/mihomo/mihomo.exe", BaseDirectory::Resource)
        {
            if resource_path.exists() {
                candidates.push(resource_path);
            }
        }
        if let Ok(resource_path) = app
            .path()
            .resolve("bin/mihomo/mihomo-windows-amd64-v3.exe", BaseDirectory::Resource)
        {
            if resource_path.exists() {
                candidates.push(resource_path);
            }
        }

        candidates
    }
}
