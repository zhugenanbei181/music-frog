//! HTTP adapter for subscription downloads.
//!
//! The subscription value objects, parsers, decoders, quota calculations and
//! security audit live in [`infiltrator_domain::subscription`]. This module is
//! deliberately limited to the transport concerns that require the injected
//! HTTP client: response-body limits, retries, header extraction and request
//! construction.

use anyhow::{Result, anyhow};
use infiltrator_domain::subscription::{
    CheckedSubscriptionUrl, SubscriptionFetchOptions, SubscriptionUserInfo, UserAgentCatalog,
    WafChallengeDetector as DomainWafChallengeDetector, WafDiagnostic, WafResponseMetadata,
    decode_subscription_bytes, parse_subscription_userinfo,
};
use infiltrator_http::HttpClient;
use infiltrator_http::reqwest::{Response, header::HeaderMap};

/// 订阅本质上是配置文件；上限只为拦截把下载接口当无限代理用的滥用。
const MAX_SUBSCRIPTION_BYTES: usize = 32 * 1024 * 1024;

/// Fetch the subscription text through the normal client and retry once with
/// the raw client when the provider returns a non-success status.
pub async fn fetch_subscription_text(
    client: &HttpClient,
    raw_client: &HttpClient,
    url: &CheckedSubscriptionUrl,
) -> Result<String> {
    fetch_subscription_with_info(client, raw_client, url)
        .await
        .map(|(text, _)| text)
}

/// Fetch the subscription body together with the provider traffic metadata
/// from the `subscription-userinfo` response header.
pub async fn fetch_subscription_with_info(
    client: &HttpClient,
    raw_client: &HttpClient,
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
            return Err(anyhow!("订阅请求被拦截 [{}]: {}", status, diag.summary));
        }
        return Err(anyhow!("订阅链接请求失败: HTTP {}", resp.status()));
    }

    let userinfo = resp
        .headers()
        .get("subscription-userinfo")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_subscription_userinfo);

    let encoding = content_encoding(resp.headers());
    let bytes = read_body_capped(resp).await?;
    let decoded_bytes = decode_subscription_bytes(bytes, encoding)?;
    let text = String::from_utf8(decoded_bytes).map_err(|e| anyhow!("UTF-8 编码错误: {}", e))?;

    if DomainWafChallengeDetector::is_html_disguised(&text) {
        return Err(anyhow!(
            "订阅返回了 HTML 网页内容而非节点配置，可能已被防爬/5秒盾拦截或套餐已过期"
        ));
    }

    Ok((text, userinfo))
}

/// Advanced subscription fetcher supporting custom headers, anti-scraping
/// camouflage, and mirror fallbacks.
pub async fn fetch_subscription_advanced(
    client: &HttpClient,
    raw_client: &HttpClient,
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
            UserAgentCatalog::find_by_id(preset)
                .map(|p| p.header)
                .unwrap_or_else(UserAgentCatalog::default_user_agent)
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
                // Fallback to raw client.
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

        let encoding = content_encoding(resp.headers());
        let bytes = read_body_capped(resp).await?;
        let decoded_bytes = decode_subscription_bytes(bytes, encoding)?;
        let text =
            String::from_utf8(decoded_bytes).map_err(|e| anyhow!("UTF-8 编码错误: {}", e))?;

        if DomainWafChallengeDetector::is_html_disguised(&text) {
            let diag = WafChallengeDetector::inspect_response(200, &HeaderMap::new(), &text);
            last_err = anyhow!("订阅返回了网页 HTML 内容: {}", diag.summary);
            continue;
        }

        return Ok((text, userinfo, None));
    }

    Err(last_err)
}

fn content_encoding(headers: &HeaderMap) -> Option<&'static str> {
    headers
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
        })
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

/// HTTP-specific WAF adapter. The classifier itself is domain logic; only
/// HeaderMap-to-values conversion belongs here.
pub struct WafChallengeDetector;

impl WafChallengeDetector {
    pub fn inspect_response(
        status_code: u16,
        headers: &HeaderMap,
        body_sample: &str,
    ) -> WafDiagnostic {
        let metadata = WafResponseMetadata {
            server_header: headers
                .get("server")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned),
            cf_ray: headers
                .get("cf-ray")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned),
            retry_after_secs: headers
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok()),
        };
        DomainWafChallengeDetector::inspect_response(status_code, &metadata, body_sample)
    }

    pub fn is_html_disguised(body: &str) -> bool {
        DomainWafChallengeDetector::is_html_disguised(body)
    }
}
