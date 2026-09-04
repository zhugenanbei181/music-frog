use anyhow::anyhow;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_core::apply::{
    ApplyOutcome, ApplyParams, ApplyStrategy, EndpointConfigReloader, apply_current_profile,
};
use infiltrator_core::session::ProfileEndpointSource;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::endpoint::EndpointSource;
use mihomo_api::client::MihomoClient;
use mihomo_api::proxy::{manager::ProxyManager, types::ProxyGroup};
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use mihomo_version::manager::VersionManager;
use reqwest::{Client, header::ACCEPT_ENCODING};
use serde::Serialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use yaml_rust2::{Yaml, YamlLoader};

use crate::service::{ServiceManager, ServiceStatus};
use crate::version;

pub struct MihomoRuntime {
    config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
    pub config_path: PathBuf,
    pub binary_path: PathBuf,
    pub controller_url: String,
    client: MihomoClient,
    service_manager: ServiceManager,
    endpoints: Arc<ProfileEndpointSource<DefaultCredentialStore>>,
    application: Arc<CoreApplication>,
    apply_guard: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MihomoSummary {
    pub profile: String,
    pub mode: String,
    pub running: bool,
    pub controller: String,
    pub groups: Vec<ProxyGroup>,
}

impl MihomoRuntime {
    /// settings `configs_dir` 覆盖；env（`INFILTRATOR_CONFIGS_DIR`）的更高
    /// 优先级在 `ConfigManager::with_configs_dir` 内部保持不变。settings
    /// 读不到（无 home / 文件缺失或损坏）按未设置处理，行为与
    /// 显式注入 home 和默认 secure store，行为与默认安装目录一致。
    async fn settings_configs_dir() -> Option<String> {
        let home = mihomo_platform::paths::get_home_dir().ok()?;
        let path = infiltrator_core::settings::settings_path(&home).ok()?;
        let settings = infiltrator_core::settings::load_settings(&path)
            .await
            .ok()?;
        settings.configs_dir
    }

    pub async fn bootstrap(
        vm: &VersionManager,
        use_bundled: bool,
        bundled_candidates: &[PathBuf],
        data_dir: &Path,
    ) -> anyhow::Result<Self> {
        let configs_dir = Self::settings_configs_dir().await;
        let home = mihomo_platform::paths::get_home_dir()?;
        let cm = Arc::new(ConfigManager::with_home_configs_dir_and_store(
            home,
            configs_dir.as_deref(),
            DefaultCredentialStore::default(),
        )?);

        cm.ensure_default_config().await?;
        cm.ensure_proxy_ports().await?;
        let controller_url = cm.ensure_external_controller().await?;
        let config_path = cm.get_current_path().await?;
        let binary = version::resolve_binary(vm, use_bundled, bundled_candidates, data_dir).await?;
        let geoip_candidates = collect_geoip_candidates(&binary, bundled_candidates);
        ensure_geoip_database(&config_path, &geoip_candidates).await?;
        let service_manager = ServiceManager::new(binary.clone(), config_path.clone());

        let endpoints = Arc::new(ProfileEndpointSource::new(cm.clone()));
        let endpoint = endpoints
            .resolve()
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        let application = Arc::new(crate::composition::core_application(
            &service_manager,
            endpoint.url.clone(),
            endpoint.secret.clone(),
        )?);
        // Attach to an already-running instance by proving it answers, or
        // start a fresh one and let the application own readiness retries.
        if service_manager.is_running().await {
            log::info!("Attaching to running mihomo service");
            application
                .adopt_if_running()
                .await
                .map_err(|error| anyhow!(error.message))?;
        } else {
            log::info!("Starting mihomo service");
            CoreLifecyclePort::start(application.as_ref())
                .await
                .map_err(|error| anyhow!(error.to_string()))?;
        }
        let client = MihomoClient::new(&endpoint.url, endpoint.secret.clone())?;

        Ok(Self {
            config_manager: cm,
            config_path,
            binary_path: binary,
            controller_url,
            client,
            service_manager,
            endpoints,
            application,
            apply_guard: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    /// The 0.30 contract/application seam for host surfaces. Legacy runtime
    /// helpers remain available while their use-cases are migrated.
    pub fn application(&self) -> Arc<CoreApplication> {
        self.application.clone()
    }

    pub fn core_binary_path(&self) -> &std::path::Path {
        &self.binary_path
    }

    pub fn tun_service_status(&self) -> crate::tun_service::ServiceModeStatus {
        crate::tun_service::TunServiceManager::check_status_for(&self.binary_path)
    }

    /// Apply the current profile's content to the core through the
    /// CORE-004 transaction: atomic write, reload-or-restart, readiness,
    /// rollback on failure.
    pub async fn apply_current_config(
        &self,
        strategy: ApplyStrategy,
    ) -> anyhow::Result<ApplyOutcome> {
        let _guard = self.apply_guard.lock().await;
        let profile = self.config_manager.get_current().await?;
        let content = self.config_manager.load(&profile).await?;
        let reloader =
            EndpointConfigReloader::new(self.endpoints.clone() as Arc<dyn EndpointSource>);
        let params = ApplyParams {
            strategy,
            ..Default::default()
        };
        let outcome = apply_current_profile(
            self.application.as_ref(),
            &self.config_manager,
            &reloader,
            &content,
            params,
        )
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
        if let Err(error) = self.config_manager.clear_backup(&profile).await {
            // The core is already healthy and the transaction has committed;
            // a stale transient backup must not turn that success into a
            // false failure (it will be overwritten by the next save).
            log::warn!("failed to clear profile apply backup: {error}");
        }
        Ok(outcome)
    }

    /// Apply caller-provided content through the same atomic write,
    /// readiness and rollback transaction used by the current profile flow.
    /// This is used by desktop editors so a running core never observes a
    /// directly-written file that has not been validated and applied.
    pub async fn apply_profile_content(
        &self,
        new_content: &str,
        strategy: ApplyStrategy,
    ) -> anyhow::Result<ApplyOutcome> {
        let _guard = self.apply_guard.lock().await;
        let profile = self.config_manager.get_current().await?;
        let reloader =
            EndpointConfigReloader::new(self.endpoints.clone() as Arc<dyn EndpointSource>);
        let params = ApplyParams {
            strategy,
            ..Default::default()
        };
        apply_current_profile(
            self.application.as_ref(),
            &self.config_manager,
            &reloader,
            new_content,
            params,
        )
        .await
        .map_err(|error| anyhow!("profile {profile}: {error}"))
    }

    pub fn client(&self) -> MihomoClient {
        self.client.clone()
    }

    pub async fn summary(&self) -> anyhow::Result<MihomoSummary> {
        let profile = self.config_manager.get_current().await?;
        let mode = self.read_mode(&profile).await?;
        let running = matches!(
            self.service_manager.status().await?,
            ServiceStatus::Running(_)
        );

        let proxy_manager = ProxyManager::new(self.client());
        let groups = proxy_manager.list_groups().await.unwrap_or_default();

        Ok(MihomoSummary {
            profile,
            mode,
            running,
            controller: self.controller_url.clone(),
            groups,
        })
    }

    pub async fn current_mode(&self) -> anyhow::Result<String> {
        let config = self.client.get_config().await?;
        Ok(normalize_mode(&config.mode))
    }

    pub async fn set_mode(&self, mode: &str) -> anyhow::Result<()> {
        let normalized = normalize_mode(mode);
        if !is_supported_mode(&normalized) {
            return Err(anyhow!("不支持的代理模式: {}", mode));
        }
        self.client
            .patch_config(json!({ "mode": normalized }))
            .await
            .map_err(|err| anyhow!(err.to_string()))
    }

    pub async fn switch_proxy(&self, group: &str, proxy: &str) -> anyhow::Result<()> {
        self.client
            .switch_proxy(group, proxy)
            .await
            .map_err(|err| anyhow!(err.to_string()))
    }

    pub async fn set_tun_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        self.client
            .patch_config(json!({ "tun": { "enable": enabled } }))
            .await
            .map_err(|err| anyhow!(err.to_string()))
    }

    async fn read_mode(&self, profile: &str) -> anyhow::Result<String> {
        let content = self.config_manager.load(profile).await?;
        let doc = parse_yaml_doc(&content)?;
        Ok(get_yaml_str(&doc, "mode").unwrap_or("rule").to_string())
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        CoreLifecyclePort::stop(self.application.as_ref())
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn is_running(&self) -> bool {
        self.service_manager.is_running().await
    }

    pub async fn http_proxy_endpoint(&self) -> anyhow::Result<Option<String>> {
        let content = tokio::fs::read_to_string(&self.config_path).await?;
        proxy_endpoint_from_config(&content)
    }
}

fn proxy_endpoint_from_config(content: &str) -> anyhow::Result<Option<String>> {
    let doc = parse_yaml_doc(content)?;
    let port = get_yaml_u16(&doc, "mixed-port").or_else(|| get_yaml_u16(&doc, "port"));
    Ok(port
        .filter(|port| *port != 0)
        .map(|p| format!("127.0.0.1:{p}")))
}

fn normalize_mode(mode: &str) -> String {
    let trimmed = mode.trim();
    if trimmed.is_empty() {
        "rule".to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn is_supported_mode(mode: &str) -> bool {
    matches!(mode, "rule" | "global" | "direct" | "script")
}

const GEOIP_URL: &str =
    "https://github.com/MetaCubeX/meta-rules-dat/releases/latest/download/geoip.metadb";
const GEOIP_MIRROR_URLS: [&str; 2] = [
    "https://fastly.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geoip.metadb",
    "https://cdn.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geoip.metadb",
];
const GEOIP_MIN_SIZE: u64 = 1024 * 1024;

fn collect_geoip_candidates(binary: &Path, bundled_candidates: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = binary.parent() {
        dirs.push(dir.to_path_buf());
    }
    for candidate in bundled_candidates {
        if let Some(dir) = candidate.parent()
            && !dirs.contains(&dir.to_path_buf())
        {
            dirs.push(dir.to_path_buf());
        }
    }

    dirs.into_iter()
        .map(|dir| dir.join("geoip.metadb"))
        .collect()
}

async fn ensure_geoip_database(
    config_path: &Path,
    geoip_candidates: &[PathBuf],
) -> anyhow::Result<()> {
    let content = tokio::fs::read_to_string(config_path).await?;
    if !content.to_ascii_uppercase().contains("GEOIP") {
        return Ok(());
    }

    let config_dir = config_path
        .parent()
        .ok_or_else(|| anyhow!("配置目录不存在"))?;
    let geoip_path = config_dir.join("geoip.metadb");
    if let Ok(meta) = tokio::fs::metadata(&geoip_path).await
        && meta.len() >= GEOIP_MIN_SIZE
    {
        return Ok(());
    }

    if try_copy_geoip_candidates(geoip_candidates, &geoip_path).await? {
        return Ok(());
    }

    let url_list = build_geoip_url_list();
    let client = Client::new();
    let mut last_err: Option<String> = None;
    for url in url_list {
        match download_geoip(&client, &url).await {
            Ok(bytes) => {
                if bytes.len() as u64 <= GEOIP_MIN_SIZE {
                    last_err = Some(format!("下载 {} 返回 {} bytes", url, bytes.len()));
                    continue;
                }
                tokio::fs::write(&geoip_path, &bytes).await?;
                return Ok(());
            }
            Err(err) => {
                last_err = Some(format!("{url}: {err}"));
            }
        }
    }

    Err(anyhow!(
        "无法获取 GeoIP 数据库：{}。请检查网络，或手动放置到 {}（也可放在内核同目录 geoip.metadb）",
        last_err.unwrap_or_else(|| "未知错误".to_string()),
        geoip_path.display()
    ))
}

fn build_geoip_url_list() -> Vec<String> {
    if let Ok(url) = std::env::var("MIHOMO_GEOIP_URL") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return vec![trimmed.to_string()];
        }
    }

    let mut urls = vec![GEOIP_URL.to_string()];
    urls.extend(GEOIP_MIRROR_URLS.iter().map(|url| url.to_string()));
    urls
}

async fn try_copy_geoip_candidates(
    candidates: &[PathBuf],
    geoip_path: &Path,
) -> anyhow::Result<bool> {
    for candidate in candidates {
        if let Ok(meta) = tokio::fs::metadata(candidate).await
            && meta.len() >= GEOIP_MIN_SIZE
        {
            log::info!(
                "使用本地 GeoIP 数据库: {} -> {}",
                candidate.display(),
                geoip_path.display()
            );
            tokio::fs::copy(candidate, geoip_path).await?;
            return Ok(true);
        }
    }

    Ok(false)
}

async fn download_geoip(client: &Client, url: &str) -> anyhow::Result<Vec<u8>> {
    let response = client
        .get(url)
        .header(ACCEPT_ENCODING, "identity")
        .header("User-Agent", "MusicFrog-Despicable-Infiltrator")
        .send()
        .await
        .map_err(|e| anyhow!("下载 GeoIP 数据库失败: {e}"))?;
    if !response.status().is_success() {
        return Err(anyhow!("下载 GeoIP 数据库失败: HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| anyhow!("读取 GeoIP 数据库失败: {e}"))?
        .to_vec();
    Ok(bytes)
}

fn parse_yaml_doc(content: &str) -> anyhow::Result<Yaml> {
    let docs = YamlLoader::load_from_str(content).map_err(|err| anyhow!(err.to_string()))?;
    docs.into_iter()
        .next()
        .ok_or_else(|| anyhow!("配置内容不是有效的 YAML"))
}

fn get_yaml_str<'a>(doc: &'a Yaml, key: &str) -> Option<&'a str> {
    doc.as_hash()?
        .get(&Yaml::String(key.to_string()))
        .and_then(|value| value.as_str())
}

fn get_yaml_u16(doc: &Yaml, key: &str) -> Option<u16> {
    let value = doc.as_hash()?.get(&Yaml::String(key.to_string()))?;
    match value {
        Yaml::Integer(num) => {
            if *num >= 0 && *num <= u16::MAX as i64 {
                Some(*num as u16)
            } else {
                None
            }
        }
        Yaml::Real(raw) => raw.parse::<u16>().ok(),
        Yaml::String(raw) => raw.parse::<u16>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_mode() {
        assert_eq!(normalize_mode("rule"), "rule");
        assert_eq!(normalize_mode("RULE"), "rule");
        assert_eq!(normalize_mode("Rule"), "rule");
        assert_eq!(normalize_mode("  rule  "), "rule");
    }

    #[test]
    fn test_normalize_mode_empty() {
        assert_eq!(normalize_mode(""), "rule");
        assert_eq!(normalize_mode("   "), "rule");
    }

    #[test]
    fn test_normalize_mode_other_modes() {
        assert_eq!(normalize_mode("global"), "global");
        assert_eq!(normalize_mode("direct"), "direct");
        assert_eq!(normalize_mode("script"), "script");
    }

    #[test]
    fn test_is_supported_mode() {
        assert!(is_supported_mode("rule"));
        assert!(is_supported_mode("global"));
        assert!(is_supported_mode("direct"));
        assert!(is_supported_mode("script"));
    }

    #[test]
    fn test_is_supported_mode_invalid() {
        assert!(!is_supported_mode("invalid"));
        assert!(!is_supported_mode(""));
        assert!(!is_supported_mode("other"));
    }

    #[test]
    fn test_parse_yaml_doc_success() {
        let yaml = r#"
port: 7890
mode: rule
"#;
        let result = parse_yaml_doc(yaml);
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert!(doc.is_hash());
    }

    #[test]
    fn test_parse_yaml_doc_invalid() {
        let yaml = "invalid: yaml: [";
        let result = parse_yaml_doc(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_yaml_doc_empty() {
        let yaml = "";
        let result = parse_yaml_doc(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_yaml_str_success() {
        let yaml = r#"
port: 7890
mode: rule
"#;
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_str(&doc, "mode");
        assert_eq!(result, Some("rule"));
    }

    #[test]
    fn test_get_yaml_str_not_found() {
        let yaml = "port: 7890";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_str(&doc, "mode");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_yaml_str_invalid_type() {
        let yaml = "port: 7890";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_str(&doc, "port");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_yaml_u16_integer() {
        let yaml = "port: 7890";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_u16(&doc, "port");
        assert_eq!(result, Some(7890));
    }

    #[test]
    fn test_get_yaml_u16_string() {
        let yaml = "port: \"7890\"";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_u16(&doc, "port");
        assert_eq!(result, Some(7890));
    }

    #[test]
    fn test_get_yaml_u16_not_found() {
        let yaml = "mode: rule";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_u16(&doc, "port");
        assert_eq!(result, None);
    }

    #[test]
    fn test_proxy_endpoint_uses_configured_non_default_port() {
        assert_eq!(
            proxy_endpoint_from_config("mixed-port: 18080\nport: 7890\n").unwrap(),
            Some("127.0.0.1:18080".to_string())
        );
        assert_eq!(
            proxy_endpoint_from_config("port: 19090\n").unwrap(),
            Some("127.0.0.1:19090".to_string())
        );
        assert_eq!(proxy_endpoint_from_config("mixed-port: 0\n").unwrap(), None);
    }

    #[test]
    fn test_collect_geoip_candidates() {
        let binary = PathBuf::from("/path/to/mihomo.exe");
        let bundled = vec![
            PathBuf::from("/path/to/bundled1/mihomo.exe"),
            PathBuf::from("/other/path/mihomo.exe"),
        ];

        let result = collect_geoip_candidates(&binary, &bundled);

        assert_eq!(result.len(), 3);
        assert!(result.contains(&PathBuf::from("/path/to/geoip.metadb")));
        assert!(result.contains(&PathBuf::from("/path/to/bundled1/geoip.metadb")));
        assert!(result.contains(&PathBuf::from("/other/path/geoip.metadb")));
    }

    #[test]
    fn test_collect_geoip_candidates_empty() {
        let binary = PathBuf::from("/path/to/mihomo.exe");
        let bundled: Vec<PathBuf> = vec![];

        let result = collect_geoip_candidates(&binary, &bundled);

        assert_eq!(result.len(), 1);
        assert!(result.contains(&PathBuf::from("/path/to/geoip.metadb")));
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_build_geoip_url_list_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("MIHOMO_GEOIP_URL") };
        let result = build_geoip_url_list();
        assert_eq!(result.len(), 3);
        assert!(result[0].contains("github.com"));
        assert!(result[1].contains("jsdelivr.net"));
    }

    #[test]
    fn test_build_geoip_url_list_custom() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("MIHOMO_GEOIP_URL", "https://custom.url/geoip.metadb") };
        let result = build_geoip_url_list();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "https://custom.url/geoip.metadb");
        unsafe { std::env::remove_var("MIHOMO_GEOIP_URL") };
    }

    #[test]
    fn test_build_geoip_url_list_empty_custom() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("MIHOMO_GEOIP_URL", "   ") };
        let result = build_geoip_url_list();
        assert_eq!(result.len(), 3);
        unsafe { std::env::remove_var("MIHOMO_GEOIP_URL") };
    }

    #[test]
    fn test_mihomo_summary_serialization() {
        let summary = MihomoSummary {
            profile: "default".to_string(),
            mode: "rule".to_string(),
            running: true,
            controller: "http://127.0.0.1:9090".to_string(),
            groups: vec![],
        };

        let serialized = serde_json::to_string(&summary);
        assert!(serialized.is_ok());
    }

    #[test]
    fn test_mihomo_summary_debug() {
        let summary = MihomoSummary {
            profile: "default".to_string(),
            mode: "rule".to_string(),
            running: true,
            controller: "http://127.0.0.1:9090".to_string(),
            groups: vec![],
        };

        let debug_str = format!("{:?}", summary);
        assert!(debug_str.contains("default"));
        assert!(debug_str.contains("rule"));
    }
}
