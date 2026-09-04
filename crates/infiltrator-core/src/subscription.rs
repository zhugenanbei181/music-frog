//! Subscription download, decoding, quota tracking, and security auditing for Mihomo / Clash.Meta.

use anyhow::{Result, anyhow};
use flate2::read::{GzDecoder, ZlibDecoder};
use infiltrator_http::reqwest::Response;
use serde::{Deserialize, Serialize};
use std::io::Read;

/// 订阅本质上是配置文件；上限只为拦截把下载接口当无限代理用的滥用。
const MAX_SUBSCRIPTION_BYTES: usize = 32 * 1024 * 1024;

/// 已通过安全校验的订阅地址：类型系统保证未经 [`CheckedSubscriptionUrl::parse`]
/// 的字符串无法进入 [`fetch_subscription_text`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSubscriptionUrl(String);

impl CheckedSubscriptionUrl {
    pub fn parse(url: &str) -> Result<Self> {
        validate_subscription_url(url)?;
        Ok(Self(url.trim().to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub async fn fetch_subscription_text(
    client: &infiltrator_http::HttpClient,
    raw_client: &infiltrator_http::HttpClient,
    url: &CheckedSubscriptionUrl,
) -> Result<String> {
    fetch_subscription_with_info(client, raw_client, url)
        .await
        .map(|(text, _)| text)
}

/// Fetch the subscription body together with the provider traffic metadata
/// from the `subscription-userinfo` response header.
pub async fn fetch_subscription_with_info(
    client: &infiltrator_http::HttpClient,
    raw_client: &infiltrator_http::HttpClient,
    url: &CheckedSubscriptionUrl,
) -> Result<(String, Option<SubscriptionUserInfo>)> {
    let mut resp = client.get(url.as_str()).send().await?;
    if !resp.status().is_success() {
        resp = raw_client.get(url.as_str()).send().await?;
    }

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let diag = WafChallengeDetector::inspect_response(status, resp.headers(), "");
        if diag.is_challenge {
            return Err(anyhow!(
                "订阅请求被拦截 [{}]: {}",
                status,
                diag.summary
            ));
        }
        return Err(anyhow!("订阅链接请求失败: HTTP {}", resp.status()));
    }

    let userinfo = resp
        .headers()
        .get("subscription-userinfo")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_subscription_userinfo);

    let encoding = resp
        .headers()
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            if v.contains("gzip") {
                "gzip"
            } else if v.contains("deflate") {
                "deflate"
            } else {
                "plain"
            }
        });

    let bytes = read_body_capped(resp).await?;
    let decoded_bytes = decode_subscription_bytes(bytes, encoding)?;
    let text = String::from_utf8(decoded_bytes).map_err(|e| anyhow!("UTF-8 编码错误: {}", e))?;

    if WafChallengeDetector::is_html_disguised(&text) {
        return Err(anyhow!(
            "订阅返回了 HTML 网页内容而非节点配置，可能已被防爬/5秒盾拦截或套餐已过期"
        ));
    }

    Ok((text, userinfo))
}

/// Extended options for subscription fetching with custom User-Agent, headers, cookies, and retry.
#[derive(Debug, Clone, Default)]
pub struct SubscriptionFetchOptions {
    pub user_agent: Option<String>,
    pub user_agent_preset: Option<String>,
    pub custom_headers: Vec<(String, String)>,
    pub cookies: Option<String>,
    pub fallback_mirrors: Vec<CheckedSubscriptionUrl>,
    pub max_retries: usize,
}

/// Advanced subscription fetcher supporting custom headers, anti-scraping camouflage, and mirror fallbacks.
pub async fn fetch_subscription_advanced(
    client: &infiltrator_http::HttpClient,
    raw_client: &infiltrator_http::HttpClient,
    url: &CheckedSubscriptionUrl,
    options: &SubscriptionFetchOptions,
) -> Result<(String, Option<SubscriptionUserInfo>, Option<WafDiagnostic>)> {
    let mut candidate_urls = vec![url.clone()];
    candidate_urls.extend(options.fallback_mirrors.clone());

    let mut last_err = anyhow!("无可用订阅地址");

    for cand_url in candidate_urls {
        let ua = if let Some(ref custom_ua) = options.user_agent {
            custom_ua.as_str()
        } else if let Some(ref preset) = options.user_agent_preset {
            UserAgentCatalog::find_by_id(preset).map(|p| p.header).unwrap_or_else(|| UserAgentCatalog::default_user_agent())
        } else {
            UserAgentCatalog::smart_user_agent_for_url(cand_url.as_str())
        };

        let mut req = client.get(cand_url.as_str()).header("User-Agent", ua);
        for (k, v) in &options.custom_headers {
            req = req.header(k.as_str(), v.as_str());
        }
        if let Some(ref cookie_str) = options.cookies {
            req = req.header("Cookie", cookie_str.as_str());
        }

        let resp_res = req.send().await;
        let resp = match resp_res {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status().as_u16();
                let headers = r.headers().clone();
                let sample_bytes = read_body_capped(r).await.unwrap_or_default();
                let sample_text = String::from_utf8_lossy(&sample_bytes);
                let diag = WafChallengeDetector::inspect_response(status, &headers, &sample_text);
                last_err = anyhow!("请求失败 HTTP {}: {}", status, diag.summary);
                continue;
            }
            Err(e) => {
                // Fallback to raw client
                let mut raw_req = raw_client.get(cand_url.as_str()).header("User-Agent", ua);
                for (k, v) in &options.custom_headers {
                    raw_req = raw_req.header(k.as_str(), v.as_str());
                }
                match raw_req.send().await {
                    Ok(r) if r.status().is_success() => r,
                    Ok(r) => {
                        let status = r.status().as_u16();
                        last_err = anyhow!("Raw 请求失败 HTTP {}", status);
                        continue;
                    }
                    Err(raw_e) => {
                        last_err = anyhow!("网络连接失败: {} / {}", e, raw_e);
                        continue;
                    }
                }
            }
        };

        let userinfo = resp
            .headers()
            .get("subscription-userinfo")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_subscription_userinfo);

        let encoding = resp
            .headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok())
            .map(|v| {
                if v.contains("gzip") {
                    "gzip"
                } else if v.contains("deflate") {
                    "deflate"
                } else {
                    "plain"
                }
            });

        let bytes = read_body_capped(resp).await?;
        let decoded_bytes = decode_subscription_bytes(bytes, encoding)?;
        let text = String::from_utf8(decoded_bytes).map_err(|e| anyhow!("UTF-8 编码错误: {}", e))?;

        if WafChallengeDetector::is_html_disguised(&text) {
            let diag = WafChallengeDetector::inspect_response(200, &infiltrator_http::reqwest::header::HeaderMap::new(), &text);
            last_err = anyhow!("订阅返回了网页 HTML 内容: {}", diag.summary);
            continue;
        }

        return Ok((text, userinfo, None));
    }

    Err(last_err)
}

/// Provider traffic metadata advertised via `subscription-userinfo`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionUserInfo {
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    /// Unix timestamp (seconds) when the plan expires.
    pub expire: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuotaStatus {
    Normal,
    NearExhaustion,
    Critical,
    Exhausted,
    Expired,
    ExpiringSoon { days_left: i64 },
}

impl SubscriptionUserInfo {
    pub fn used_bytes(&self) -> u64 {
        self.upload.unwrap_or(0).saturating_add(self.download.unwrap_or(0))
    }

    pub fn usage_percentage(&self) -> Option<f64> {
        let total = self.total?;
        if total == 0 {
            return None;
        }
        let used = self.used_bytes();
        let pct = (used as f64 / total as f64) * 100.0;
        Some((pct * 10.0).round() / 10.0)
    }

    pub fn remaining_bytes(&self) -> Option<u64> {
        let total = self.total?;
        let used = self.used_bytes();
        Some(total.saturating_sub(used))
    }

    pub fn remaining_days(&self, now_unix: i64) -> Option<i64> {
        let expire = self.expire?;
        let diff_secs = expire.saturating_sub(now_unix);
        Some(diff_secs / 86400)
    }

    pub fn status(&self, now_unix: i64) -> QuotaStatus {
        self.status_with_thresholds(now_unix, 80.0, 90.0, 3)
    }

    pub fn status_with_thresholds(
        &self,
        now_unix: i64,
        near_exhaustion_pct: f64,
        critical_pct: f64,
        expiring_soon_days: i64,
    ) -> QuotaStatus {
        if let Some(exp) = self.expire
            && exp <= now_unix
        {
            return QuotaStatus::Expired;
        }

        if let Some(rem) = self.remaining_bytes()
            && rem == 0
            && self.total.is_some()
        {
            return QuotaStatus::Exhausted;
        }

        if let Some(exp) = self.expire {
            let diff_secs = exp.saturating_sub(now_unix);
            let days_left = diff_secs / 86400;
            if days_left <= expiring_soon_days {
                return QuotaStatus::ExpiringSoon { days_left };
            }
        }

        if let Some(pct) = self.usage_percentage() {
            if pct >= critical_pct {
                return QuotaStatus::Critical;
            }
            if pct >= near_exhaustion_pct {
                return QuotaStatus::NearExhaustion;
            }
        }

        QuotaStatus::Normal
    }

    /// Formats a byte count into a human-readable string (B, KB, MB, GB, TB, PB).
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;
        const PB: u64 = TB * 1024;

        if bytes >= PB {
            format!("{:.2} PB", bytes as f64 / PB as f64)
        } else if bytes >= TB {
            format!("{:.2} TB", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{bytes} B")
        }
    }

    pub fn format_used(&self) -> String {
        Self::format_bytes(self.used_bytes())
    }

    pub fn format_total(&self) -> String {
        self.total.map(Self::format_bytes).unwrap_or_else(|| "无限".into())
    }

    pub fn format_remaining(&self) -> String {
        self.remaining_bytes().map(Self::format_bytes).unwrap_or_else(|| "未知".into())
    }

    /// Calculates daily consumption burn-rate (in GB/day) based on previous snapshot.
    pub fn burn_rate_gb_per_day(
        &self,
        previous_info: &SubscriptionUserInfo,
        elapsed_secs: i64,
    ) -> Option<f64> {
        if elapsed_secs <= 0 {
            return None;
        }
        let current_used = self.used_bytes();
        let prev_used = previous_info.used_bytes();
        if current_used < prev_used {
            return None; // Plan reset
        }
        let delta_bytes = current_used.saturating_sub(prev_used);
        let delta_gb = delta_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let days = elapsed_secs as f64 / 86400.0;
        Some((delta_gb / days * 100.0).round() / 100.0)
    }

    /// Predicts unix timestamp when remaining quota will be exhausted at current burn rate.
    pub fn projected_exhaustion_unix(
        &self,
        previous_info: &SubscriptionUserInfo,
        elapsed_secs: i64,
        now_unix: i64,
    ) -> Option<i64> {
        let remaining_bytes = self.remaining_bytes()?;
        if remaining_bytes == 0 {
            return Some(now_unix);
        }
        if elapsed_secs <= 0 {
            return None;
        }
        let current_used = self.used_bytes();
        let prev_used = previous_info.used_bytes();
        if current_used <= prev_used {
            return None;
        }
        let bytes_per_sec = (current_used - prev_used) as f64 / elapsed_secs as f64;
        if bytes_per_sec <= 0.0 {
            return None;
        }
        let secs_until_exhaustion = (remaining_bytes as f64 / bytes_per_sec) as i64;
        Some(now_unix.saturating_add(secs_until_exhaustion))
    }
}

/// Parse the `k=v; k=v` header form.
pub fn parse_subscription_userinfo(raw: &str) -> Option<SubscriptionUserInfo> {
    let mut info = SubscriptionUserInfo::default();
    for pair in raw.split(';') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "upload" => info.upload = value.parse().ok(),
            "download" => info.download = value.parse().ok(),
            "total" => info.total = value.parse().ok(),
            "expire" => info.expire = value.parse().ok(),
            _ => {}
        }
    }
    let empty = info.upload.is_none()
        && info.download.is_none()
        && info.total.is_none()
        && info.expire.is_none();
    if empty { None } else { Some(info) }
}

/// Pre-configured User-Agent presets to avoid ISP or provider blocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserAgentPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub header: &'static str,
}

pub struct UserAgentCatalog;

impl UserAgentCatalog {
    pub const PRESETS: &'static [UserAgentPreset] = &[
        UserAgentPreset {
            id: "clash-meta",
            name: "Clash.Meta",
            header: "Clash.Meta; mihomo/1.19.0",
        },
        UserAgentPreset {
            id: "clash-verge",
            name: "Clash Verge Rev",
            header: "ClashVerge/1.7.0 (Clash.Meta)",
        },
        UserAgentPreset {
            id: "shadowrocket",
            name: "Shadowrocket",
            header: "Shadowrocket/2.2.34 (iOS; iPhone15,2; iOS 17.5.1)",
        },
        UserAgentPreset {
            id: "quantumult-x",
            name: "Quantumult X",
            header: "Quantumult%20X/1.0.31 (iPhone; iOS 17.4)",
        },
        UserAgentPreset {
            id: "surge",
            name: "Surge",
            header: "Surge/2800 (macOS; 14.5; arm64)",
        },
        UserAgentPreset {
            id: "loon",
            name: "Loon",
            header: "Loon/643 (iOS; 17.5; iPhone16,1)",
        },
        UserAgentPreset {
            id: "stash",
            name: "Stash",
            header: "Stash/2.6.2 (iOS; 17.5; iPhone15,3)",
        },
        UserAgentPreset {
            id: "sing-box",
            name: "sing-box",
            header: "sing-box/1.9.0 (Linux; x86_64)",
        },
        UserAgentPreset {
            id: "chrome-windows",
            name: "Chrome (Windows)",
            header: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        },
        UserAgentPreset {
            id: "chrome-macos",
            name: "Chrome (macOS)",
            header: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        },
        UserAgentPreset {
            id: "safari-ios",
            name: "Safari (iOS)",
            header: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
        },
        UserAgentPreset {
            id: "firefox-linux",
            name: "Firefox (Linux)",
            header: "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
        },
    ];

    pub fn get_all() -> &'static [UserAgentPreset] {
        Self::PRESETS
    }

    pub fn default_user_agent() -> &'static str {
        "Clash.Meta; MusicFrog/0.20.0"
    }

    pub fn find_by_id(id: &str) -> Option<&'static UserAgentPreset> {
        Self::PRESETS.iter().find(|p| p.id.eq_ignore_ascii_case(id))
    }

    pub fn smart_user_agent_for_url(url: &str) -> &'static str {
        let lower = url.to_ascii_lowercase();
        if lower.contains("sub") || lower.contains("clash") {
            "Clash.Meta; mihomo/1.19.0"
        } else if lower.contains("shadowrocket") {
            "Shadowrocket/2.2.34 (iOS; iPhone15,2; iOS 17.5.1)"
        } else if lower.contains("surge") {
            "Surge/2800 (macOS; 14.5; arm64)"
        } else {
            Self::default_user_agent()
        }
    }

    pub fn get_browser_headers_for(_preset_id: &str) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"),
            ("Accept-Language", "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7"),
            ("Sec-CH-UA", "\"Chromium\";v=\"128\", \"Not;A=Brand\";v=\"24\", \"Google Chrome\";v=\"128\""),
            ("Sec-CH-UA-Mobile", "?0"),
            ("Sec-CH-UA-Platform", "\"Windows\""),
            ("Sec-Fetch-Dest", "document"),
            ("Sec-Fetch-Mode", "navigate"),
            ("Sec-Fetch-Site", "none"),
            ("Sec-Fetch-User", "?1"),
            ("Upgrade-Insecure-Requests", "1"),
        ]
    }
}

/// Anti-scraping and WAF (Cloudflare 5-second shield) challenge detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChallengeType {
    None,
    Cloudflare5sShield,
    TurnstileCaptcha,
    Hcaptcha,
    WafRateLimited,
    WafForbidden,
    HtmlDisguisedAsConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WafDiagnostic {
    pub is_challenge: bool,
    pub challenge_type: ChallengeType,
    pub status_code: u16,
    pub server_header: Option<String>,
    pub cf_ray: Option<String>,
    pub retry_after_secs: Option<u64>,
    pub summary: String,
    pub suggestion: String,
}

pub struct WafChallengeDetector;

impl WafChallengeDetector {
    /// Inspects HTTP response status, headers, and body sample for WAF / 5s shield signatures.
    pub fn inspect_response(
        status_code: u16,
        headers: &infiltrator_http::reqwest::header::HeaderMap,
        body_sample: &str,
    ) -> WafDiagnostic {
        let server = headers
            .get("server")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let cf_ray = headers
            .get("cf-ray")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let retry_after = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        let is_cf = server.as_deref().is_some_and(|s| s.to_ascii_lowercase().contains("cloudflare")) || cf_ray.is_some();
        let body_lower = body_sample.to_ascii_lowercase();

        let (is_challenge, challenge_type, summary, suggestion) = if status_code == 429 {
            (
                true,
                ChallengeType::WafRateLimited,
                "请求频率过高，触发上游速率限制 (HTTP 429)".into(),
                "请增加更新间隔时间或等待冷却后再试".into(),
            )
        } else if body_lower.contains("just a moment")
            || body_lower.contains("checking your browser")
            || body_lower.contains("challenge-platform")
            || body_lower.contains("cf-browser-verification")
        {
            (
                true,
                ChallengeType::Cloudflare5sShield,
                "触发 Cloudflare 5秒盾浏览器验证".into(),
                "请尝试切换 User-Agent 预设 (如 Shadowrocket 或 Chrome) 或在浏览器中提取 cf_clearance Cookie 注入".into(),
            )
        } else if body_lower.contains("turnstile") || body_lower.contains("challenges.cloudflare.com") {
            (
                true,
                ChallengeType::TurnstileCaptcha,
                "遇到 Cloudflare Turnstile 人机验证码".into(),
                "需通过浏览器手动通过验证后配置 Cookie 鉴权".into(),
            )
        } else if body_lower.contains("hcaptcha") {
            (
                true,
                ChallengeType::Hcaptcha,
                "遇到 hCaptcha 验证码拦截".into(),
                "请联系机场管理员或使用备用订阅镜像".into(),
            )
        } else if (status_code == 403 || status_code == 503) && is_cf {
            (
                true,
                ChallengeType::WafForbidden,
                format!("上游 Cloudflare WAF 拒绝访问 (HTTP {status_code})"),
                "请检查 IP 是否被拉黑，或配置直连代理/伪装请求头".into(),
            )
        } else if status_code == 200 && Self::is_html_disguised(body_sample) {
            (
                true,
                ChallengeType::HtmlDisguisedAsConfig,
                "订阅返回了 HTML 页面而非有效配置 (可能是套餐过期或需登录提示)".into(),
                "请登录机场官网检查套餐是否有效，或更新订阅 Token".into(),
            )
        } else {
            (
                false,
                ChallengeType::None,
                "正常响应".into(),
                String::new(),
            )
        };

        WafDiagnostic {
            is_challenge,
            challenge_type,
            status_code,
            server_header: server,
            cf_ray,
            retry_after_secs: retry_after,
            summary,
            suggestion,
        }
    }

    /// Detects whether the downloaded content is an HTML webpage rather than a proxy config.
    pub fn is_html_disguised(body: &str) -> bool {
        let trimmed = body.trim();
        if trimmed.starts_with("<!DOCTYPE")
            || trimmed.starts_with("<!doctype")
            || trimmed.starts_with("<html")
            || trimmed.starts_with("<HTML")
            || (trimmed.contains("<head>") && trimmed.contains("<body>"))
            || (trimmed.contains("<script") && trimmed.contains("</script>"))
        {
            return true;
        }
        false
    }
}

/// Audit report for malicious configuration injection detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionAuditResult {
    pub is_safe: bool,
    pub flagged_keys: Vec<String>,
    pub warnings: Vec<String>,
}

pub struct SubscriptionSecurityAuditor;

impl SubscriptionSecurityAuditor {
    /// Inspects subscription YAML content for dangerous parameters.
    pub fn audit_subscription_content(content: &str) -> SubscriptionAuditResult {
        let mut flagged_keys = Vec::new();
        let mut warnings = Vec::new();

        if let Ok(doc) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(content)
            && let Some(map) = doc.as_mapping()
        {
            // Check external-controller binding
            if let Some(ec) = map.get(serde_yaml_ng::Value::String("external-controller".to_string()))
                && let Some(ec_str) = ec.as_str()
                && (ec_str.contains("0.0.0.0") || ec_str.starts_with(':'))
            {
                flagged_keys.push("external-controller".to_string());
                warnings.push(format!("Dangerous external-controller public exposure: {ec_str}"));
            }

            // Check secret tampering
            if map.contains_key(serde_yaml_ng::Value::String("secret".to_string())) {
                flagged_keys.push("secret".to_string());
                warnings.push("Subscription attempts to override API secret".to_string());
            }

            // Check arbitrary script blocks
            if map.contains_key(serde_yaml_ng::Value::String("script".to_string())) {
                flagged_keys.push("script".to_string());
                warnings.push("Subscription contains arbitrary executable script block".to_string());
            }

            // Check port collisions
            for key in ["port", "socks-port", "mixed-port", "redir-port", "tproxy-port"] {
                if let Some(val) = map.get(serde_yaml_ng::Value::String(key.to_string()))
                    && let Some(port) = val.as_u64()
                    && (port == 22 || port == 53 || port == 80 || port == 443)
                {
                    warnings.push(format!("Port override targeting sensitive port: {key}={port}"));
                }
            }

            // Check dangerous TUN hijacking
            if let Some(tun) = map.get(serde_yaml_ng::Value::String("tun".to_string()))
                && let Some(tun_map) = tun.as_mapping()
                && let Some(auto_route) = tun_map.get(serde_yaml_ng::Value::String("auto-route".to_string()))
                    && auto_route.as_bool() == Some(true)
                {
                    warnings.push("Subscription enables auto-route in TUN section".to_string());
                }
        }

        let is_safe = flagged_keys.is_empty();
        SubscriptionAuditResult {
            is_safe,
            flagged_keys,
            warnings,
        }
    }
}

pub fn validate_subscription_url(url: &str) -> Result<()> {
    let parsed =
        infiltrator_http::reqwest::Url::parse(url).map_err(|_| anyhow!("订阅链接格式无效"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(anyhow!("订阅链接仅支持 http/https 协议"));
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err(anyhow!("订阅链接缺少主机名"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(anyhow!("订阅链接不应内嵌用户名/密码"));
    }
    Ok(())
}

async fn read_body_capped(resp: Response) -> Result<Vec<u8>> {
    if let Some(len) = resp.content_length()
        && len as usize > MAX_SUBSCRIPTION_BYTES
    {
        return Err(anyhow!(
            "订阅内容超过大小上限 ({MAX_SUBSCRIPTION_BYTES} 字节)"
        ));
    }
    let mut resp = resp;
    let mut buffer = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        if buffer.len().saturating_add(chunk.len()) > MAX_SUBSCRIPTION_BYTES {
            return Err(anyhow!(
                "订阅内容超过大小上限 ({MAX_SUBSCRIPTION_BYTES} 字节)"
            ));
        }
        buffer.extend_from_slice(&chunk);
    }
    Ok(buffer)
}

pub fn mask_subscription_url(url: &str) -> String {
    if let Ok(mut parsed) = infiltrator_http::reqwest::Url::parse(url) {
        let path = parsed.path().to_string();
        let segments: Vec<&str> = path.split('/').collect();
        if segments.len() > 2 {
            let last = segments.last().unwrap_or(&"");
            if last.len() > 8 {
                let masked_path = path.replace(last, "***");
                parsed.set_path(&masked_path);
                return parsed.to_string();
            }
        }
    }
    url.to_string()
}

pub fn strip_utf8_bom(text: &str) -> &str {
    text.strip_prefix("\u{feff}").unwrap_or(text)
}

fn decode_subscription_bytes(bytes: Vec<u8>, encoding: Option<&str>) -> Result<Vec<u8>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mut data = bytes;
    if let Some(enc) = encoding {
        match enc {
            "gzip" => {
                let mut decoder = GzDecoder::new(&data[..]);
                let mut decoded = Vec::new();
                decoder.read_to_end(&mut decoded)?;
                data = decoded;
            }
            "deflate" => {
                let mut decoder = ZlibDecoder::new(&data[..]);
                let mut decoded = Vec::new();
                decoder.read_to_end(&mut decoded)?;
                data = decoded;
            }
            _ => {}
        }
    } else if looks_like_gzip(&data) {
        let mut decoder = GzDecoder::new(&data[..]);
        let mut decoded = Vec::new();
        if decoder.read_to_end(&mut decoded).is_ok() {
            data = decoded;
        }
    }

    Ok(data)
}

fn looks_like_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 10 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

#[cfg(test)]
#[path = "subscription_test.rs"]
mod subscription_test;
