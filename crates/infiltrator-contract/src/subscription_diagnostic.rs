//! Subscription update fetch diagnostics and node regex transformation pipeline (FLPARITY-033).
//!
//! Provides strongly-typed models for:
//! * `SubscriptionFetchDiagnostic`: detailed HTTP/TLS/DNS/WAF failure inspection with credential scrubbing;
//! * `NodeRegexRule` & `NodeRegexTransformPipeline`: regex-based node name cleaning, renaming, and exclusion.

use serde::{Deserialize, Serialize};

/// High-level diagnostic classification for subscription fetch failures.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionDiagnosticCategory {
    /// DNS lookup failure or unresolvable subscription hostname.
    DnsResolutionFailed,
    /// TLS certificate verification failure (expired, self-signed, untrusted).
    TlsCertificateError,
    /// HTTP status code error (401, 403, 404, 500, 502, 503, etc.).
    HttpError(u16),
    /// Cloudflare / WAF anti-bot challenge or captcha intercept.
    CloudflareChallenge,
    /// TCP connection refused or reset by peer.
    ConnectionRefused,
    /// Network connection or read timeout.
    Timeout,
    /// HTTP 200 received but content is invalid / unparseable format.
    MalformedContent,
    /// Unclassified network or transport failure.
    Other(String),
}

impl SubscriptionDiagnosticCategory {
    /// User-friendly Chinese diagnosis title.
    pub fn title_zh(&self) -> String {
        match self {
            Self::DnsResolutionFailed => "DNS 域名解析失败".to_owned(),
            Self::TlsCertificateError => "TLS 证书验证错误".to_owned(),
            Self::HttpError(code) => format!("HTTP 状态异常 ({code})"),
            Self::CloudflareChallenge => "Cloudflare 5秒盾 / 人机验证拦截".to_owned(),
            Self::ConnectionRefused => "TCP 连接被拒绝 / 端口阻断".to_owned(),
            Self::Timeout => "网络连接超时".to_owned(),
            Self::MalformedContent => "订阅内容格式解析失败".to_owned(),
            Self::Other(msg) => format!("网络故障: {msg}"),
        }
    }

    /// Actionable user recommendation in Chinese.
    pub fn recommendation_zh(&self) -> &'static str {
        match self {
            Self::DnsResolutionFailed => "检查当前网络 DNS 配置或尝试开启系统代理/直连切换",
            Self::TlsCertificateError => "订阅服务器证书不可信，如确需使用可勾选“跳过证书验证”",
            Self::HttpError(401 | 403) => {
                "订阅凭据可能已过期或被机场限制，请检查订阅链接与套餐配额"
            }
            Self::HttpError(code) if *code >= 500 => {
                "订阅服务提供商后端服务异常，请稍后重试或联系机场客服"
            }
            Self::HttpError(_) => "服务器返回异常状态码，请检查订阅 URL 是否完整",
            Self::CloudflareChallenge => {
                "服务商启用了防爬虫质询，建议切换自定义 User-Agent 或直连拉取"
            }
            Self::ConnectionRefused => "无法连接目标服务器，请检查节点/网络防火墙或代理设置",
            Self::Timeout => "请求超时未响应，请检查当前国际出口连通性或稍后重试",
            Self::MalformedContent => "响应非合法 Clash/Sing-Box 格式，可能已被运营商重定向或篡改",
            Self::Other(_) => "请复制诊断报告并联系服务支持",
        }
    }
}

/// Detailed diagnostic facts for a failed subscription update.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionFetchDiagnostic {
    /// Sanitized URL with access tokens and passwords scrubbed.
    pub target_url_redacted: String,
    /// Primary root cause category.
    pub category: SubscriptionDiagnosticCategory,
    /// HTTP response status code, if received.
    pub http_status: Option<u16>,
    /// Observed DNS resolve latency in milliseconds.
    pub dns_latency_ms: Option<u32>,
    /// Observed TCP connection latency in milliseconds.
    pub connect_latency_ms: Option<u32>,
    /// Negotiated TLS protocol version (e.g. "TLSv1.3").
    pub tls_version: Option<String>,
    /// Server header banner, if present.
    pub server_banner: Option<String>,
    /// Sanitized error text snippet.
    pub raw_error_snippet: String,
}

impl SubscriptionFetchDiagnostic {
    /// Scrub sensitive tokens (token, auth, password, secret) from a subscription URL.
    pub fn scrub_url(url_str: &str) -> String {
        let trimmed = url_str.trim();
        if let Some((base, query)) = trimmed.split_once('?') {
            let sanitized_params: Vec<String> = query
                .split('&')
                .map(|pair| {
                    if let Some((k, _)) = pair.split_once('=') {
                        let key_lower = k.to_ascii_lowercase();
                        if key_lower.contains("token")
                            || key_lower.contains("secret")
                            || key_lower.contains("auth")
                            || key_lower.contains("pass")
                            || key_lower.contains("key")
                        {
                            format!("{k}=***")
                        } else {
                            pair.to_owned()
                        }
                    } else {
                        pair.to_owned()
                    }
                })
                .collect();
            format!("{base}?{}", sanitized_params.join("&"))
        } else {
            trimmed.to_owned()
        }
    }

    /// Construct a formatted, copyable markdown diagnostic report.
    pub fn to_markdown_report(&self) -> String {
        let mut report = String::with_capacity(512);
        report.push_str("### 订阅更新故障诊断报告\n\n");
        report.push_str(&format!("- **目标地址**: `{}`\n", self.target_url_redacted));
        report.push_str(&format!("- **故障分类**: {}\n", self.category.title_zh()));
        if let Some(status) = self.http_status {
            report.push_str(&format!("- **HTTP 状态码**: {status}\n"));
        }
        if let Some(ms) = self.dns_latency_ms {
            report.push_str(&format!("- **DNS 解析延迟**: {ms} ms\n"));
        }
        if let Some(ms) = self.connect_latency_ms {
            report.push_str(&format!("- **连接握手延迟**: {ms} ms\n"));
        }
        if let Some(tls) = &self.tls_version {
            report.push_str(&format!("- **TLS 协议**: {tls}\n"));
        }
        if let Some(server) = &self.server_banner {
            report.push_str(&format!("- **Server 标识**: {server}\n"));
        }
        report.push_str(&format!(
            "\n**排查建议**: {}\n",
            self.category.recommendation_zh()
        ));
        if !self.raw_error_snippet.is_empty() {
            report.push_str(&format!("\n```\n{}\n```\n", self.raw_error_snippet));
        }
        report
    }
}

/// A single regex cleaning or replacement rule for proxy node names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRegexRule {
    /// Regular expression pattern.
    pub pattern: String,
    /// Replacement string (used when `is_exclude` is false).
    pub replacement: String,
    /// If true, any node matching `pattern` is filtered out / dropped.
    #[serde(default)]
    pub is_exclude: bool,
}

impl NodeRegexRule {
    pub fn rename(pattern: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            replacement: replacement.into(),
            is_exclude: false,
        }
    }

    pub fn exclude(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            replacement: String::new(),
            is_exclude: true,
        }
    }
}

/// Multi-stage regex cleaning pipeline model for normalizing node names before subscription commit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRegexTransformPipeline {
    pub rules: Vec<NodeRegexRule>,
}

impl NodeRegexTransformPipeline {
    pub fn new(rules: Vec<NodeRegexRule>) -> Self {
        Self { rules }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_scrubbing_removes_sensitive_query_parameters() {
        let raw = "https://sub.example.com/api/v1/client/subscribe?token=abc123456789&flag=clash";
        let scrubbed = SubscriptionFetchDiagnostic::scrub_url(raw);
        assert!(!scrubbed.contains("abc123456789"));
        assert!(scrubbed.contains("token=***"));
        assert!(scrubbed.contains("flag=clash"));
    }

    #[test]
    fn markdown_report_generation() {
        let diag = SubscriptionFetchDiagnostic {
            target_url_redacted: "https://sub.example.com/api?token=***".to_owned(),
            category: SubscriptionDiagnosticCategory::CloudflareChallenge,
            http_status: Some(403),
            dns_latency_ms: Some(15),
            connect_latency_ms: Some(45),
            tls_version: Some("TLSv1.3".to_owned()),
            server_banner: Some("cloudflare".to_owned()),
            raw_error_snippet: "error code: 1020 access denied".to_owned(),
        };

        let report = diag.to_markdown_report();
        assert!(report.contains("Cloudflare 5秒盾"));
        assert!(report.contains("403"));
        assert!(report.contains("HTTP 状态码"));
        assert!(report.contains("error code: 1020"));
        assert!(report.contains("排查建议"));
    }
}
