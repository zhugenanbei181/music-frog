use anyhow::anyhow;
use log::{info, warn};
use reqwest::{
    header::{ACCEPT, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE},
    Client,
};
use std::io::Read;

pub async fn fetch_subscription_text(
    default_client: &Client,
    raw_client: &Client,
    url: &str,
) -> anyhow::Result<String> {
    let primary = fetch_subscription_bytes(default_client, url, false).await;
    let response = match primary {
        Ok(response) => response,
        Err(err) => {
            if is_decode_error(&err) {
                warn!("subscription decode error, retry with identity: {err}");
                fetch_subscription_bytes(raw_client, url, true).await?
            } else {
                return Err(err);
            }
        }
    };

    let bytes = if response.used_raw_client {
        decode_subscription_bytes(response.bytes, response.encoding.as_deref())?
    } else {
        response.bytes
    };

    let content = decode_utf8_text(&bytes)?;
    Ok(strip_utf8_bom(&content))
}

pub fn strip_utf8_bom(content: &str) -> String {
    content.trim_start_matches('\u{feff}').to_string()
}

pub fn mask_subscription_url(url: &str) -> String {
    let mut masked = url.to_string();
    if let Some(pos) = masked.find("link/") {
        let start = pos + "link/".len();
        if let Some(tail) = masked.get(start..) {
            let end = tail
                .find('?')
                .map(|offset| start + offset)
                .unwrap_or_else(|| masked.len());
            if start < end {
                masked.replace_range(start..end, "***");
            }
        }
    }
    masked
}

struct SubscriptionResponse {
    bytes: Vec<u8>,
    encoding: Option<String>,
    used_raw_client: bool,
}

async fn fetch_subscription_bytes(
    client: &Client,
    url: &str,
    force_identity: bool,
) -> anyhow::Result<SubscriptionResponse> {
    let mut request = client.get(url).header(ACCEPT, "text/yaml, text/plain, */*");
    if force_identity {
        request = request.header(ACCEPT_ENCODING, "identity");
    }
    let response = request.send().await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("-")
        .to_string();
    let encoding = response
        .headers()
        .get(CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let bytes = response.bytes().await?;
    let size = bytes.len();
    info!(
        "subscription response: status={} content-type={} encoding={} bytes={}",
        status.as_u16(),
        content_type,
        encoding.clone().unwrap_or_else(|| "-".to_string()),
        size
    );
    if !status.is_success() {
        return Err(anyhow!("拉取失败，HTTP {}", status));
    }
    if size == 0 {
        return Err(anyhow!("订阅返回内容为空"));
    }
    Ok(SubscriptionResponse {
        bytes: bytes.to_vec(),
        encoding,
        used_raw_client: force_identity,
    })
}

fn decode_subscription_bytes(bytes: Vec<u8>, encoding: Option<&str>) -> anyhow::Result<Vec<u8>> {
    let encoding = encoding
        .unwrap_or("")
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let encoding = if encoding.is_empty() {
        if looks_like_gzip(&bytes) {
            "gzip".to_string()
        } else {
            String::new()
        }
    } else {
        encoding
    };

    match encoding.as_str() {
        "" => Ok(bytes),
        "gzip" | "x-gzip" => decode_gzip(&bytes),
        "deflate" => decode_deflate(&bytes),
        "br" => decode_brotli(&bytes),
        other => Err(anyhow!("不支持的订阅编码类型: {}", other)),
    }
}

fn decode_gzip(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut decoder = flate2::read::GzDecoder::new(bytes);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| anyhow!("gzip 解码失败: {e}"))?;
    Ok(output)
}

fn decode_deflate(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut decoder = flate2::read::DeflateDecoder::new(bytes);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| anyhow!("deflate 解码失败: {e}"))?;
    Ok(output)
}

fn decode_brotli(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut decoder = brotli::Decompressor::new(bytes, 4096);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| anyhow!("brotli 解码失败: {e}"))?;
    Ok(output)
}

fn looks_like_gzip(bytes: &[u8]) -> bool {
    matches!(bytes.get(..2), Some([0x1f, 0x8b]))
}

fn decode_utf8_text(bytes: &[u8]) -> anyhow::Result<String> {
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => Ok(text),
        Err(err) => {
            warn!("subscription utf-8 decode failed: {err}");
            Ok(String::from_utf8_lossy(bytes).to_string())
        }
    }
}

fn is_decode_error(err: &anyhow::Error) -> bool {
    let message = err.to_string().to_ascii_lowercase();
    message.contains("error decoding response body")
        || message.contains("failed to decode")
        || message.contains("decoder")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_utf8_bom() {
        let content_with_bom = "\u{feff}hello";
        assert_eq!(strip_utf8_bom(content_with_bom), "hello");

        let content_without_bom = "hello";
        assert_eq!(strip_utf8_bom(content_without_bom), "hello");
    }

    #[test]
    fn test_mask_subscription_url() {
        let url = "https://example.com/link/abcdefg123456?mu=0";
        assert_eq!(mask_subscription_url(url), "https://example.com/link/***?mu=0");

        let url_no_query = "https://example.com/link/abcdefg123456";
        assert_eq!(mask_subscription_url(url_no_query), "https://example.com/link/***");

        let normal_url = "https://google.com";
        assert_eq!(mask_subscription_url(normal_url), "https://google.com");
    }

    #[test]
    fn test_looks_like_gzip() {
        assert!(looks_like_gzip(&[0x1f, 0x8b, 0x08]));
        assert!(!looks_like_gzip(&[0x00, 0x00, 0x00]));
        assert!(!looks_like_gzip(&[]));
    }
}
