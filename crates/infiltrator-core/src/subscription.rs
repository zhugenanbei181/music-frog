use anyhow::{Result, anyhow};
use flate2::read::{GzDecoder, ZlibDecoder};
use infiltrator_http::reqwest::Response;
use std::io::Read;

/// 订阅本质上是配置文件；上限只为拦截把下载接口当无限代理用的滥用。
const MAX_SUBSCRIPTION_BYTES: usize = 32 * 1024 * 1024;

/// 已通过安全校验的订阅地址：类型系统保证未经 [`CheckedSubscriptionUrl::parse`]
/// 的字符串无法进入 [`fetch_subscription_text`]。
#[derive(Debug, Clone)]
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
    let mut resp = client.get(url.as_str()).send().await?;
    if !resp.status().is_success() {
        resp = raw_client.get(url.as_str()).send().await?;
    }

    if !resp.status().is_success() {
        return Err(anyhow!("订阅链接请求失败: HTTP {}", resp.status()));
    }

    let encoding = resp
        .headers()
        .get("subscription-userinfo")
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
    Ok(text)
}

/// 拉取地址来自订阅配置/管理端 API，收紧到 http(s)、有主机、不带内嵌凭据，
/// 防止 file:// 等协议与凭据泄漏类滥用的入口。
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

/// 分块读取并强制大小上限，避免超 large Content-Length 响应拖垮内存。
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
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_gzip() {
        assert!(!looks_like_gzip(&[0, 1, 2]));
        let mut gzip_header = vec![0u8; 10];
        gzip_header[0] = 0x1f;
        gzip_header[1] = 0x8b;
        assert!(looks_like_gzip(&gzip_header));
    }

    #[test]
    fn test_decode_utf8_text() {
        let text = "plain text";
        let decoded = decode_subscription_bytes(text.as_bytes().to_vec(), None).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), text);
    }

    #[test]
    fn test_decode_gzip() {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"compressed").unwrap();
        let compressed = encoder.finish().unwrap();
        let decoded = decode_subscription_bytes(compressed, Some("gzip")).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "compressed");
    }

    #[test]
    fn test_decode_deflate() {
        use std::io::Write;
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"deflated").unwrap();
        let compressed = encoder.finish().unwrap();
        let decoded = decode_subscription_bytes(compressed, Some("deflate")).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "deflated");
    }

    #[test]
    fn test_mask_subscription_url() {
        let url = "https://example.com/link/abcdefg123456?mu=0";
        let masked = mask_subscription_url(url);
        assert!(masked.contains("***"));
        assert!(!masked.contains("abcdefg123456"));
    }

    #[test]
    fn test_validate_subscription_url() {
        assert!(validate_subscription_url("https://sub.example.com/token").is_ok());
        assert!(validate_subscription_url("http://192.168.1.1/sub").is_ok());
        assert!(validate_subscription_url("file:///etc/passwd").is_err());
        assert!(validate_subscription_url("ftp://example.com/sub").is_err());
        assert!(validate_subscription_url("https://user:pass@example.com/sub").is_err());
        // 特殊 scheme 会容忍多余斜杠（host 变成 "no-host"），真正无主机的是这种：
        assert!(validate_subscription_url("https://").is_err());
        assert!(validate_subscription_url("not a url").is_err());
        assert!(validate_subscription_url("").is_err());
    }

    #[test]
    fn test_strip_utf8_bom() {
        let text = "\u{feff}config content";
        assert_eq!(strip_utf8_bom(text), "config content");
        assert_eq!(strip_utf8_bom("no bom"), "no bom");
    }

    #[test]
    fn test_decode_subscription_bytes_auto_gzip() {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"hello world").unwrap();
        let compressed = encoder.finish().unwrap();

        let decoded = decode_subscription_bytes(compressed, None).unwrap();
        assert_eq!(decoded, b"hello world");
    }

    #[test]
    fn test_strip_utf8_bom_exhaustive() {
        let with_bom = vec![0xEF, 0xBB, 0xBF, b'a', b'b'];
        assert_eq!(
            strip_utf8_bom(std::str::from_utf8(&with_bom).unwrap()),
            "ab"
        );
        assert_eq!(strip_utf8_bom("no bom"), "no bom");
        assert_eq!(strip_utf8_bom(""), "");
    }

    #[test]
    fn test_looks_like_gzip_minimum_size() {
        assert!(!looks_like_gzip(&[0x1f, 0x8b]));
        assert!(looks_like_gzip(&[
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        ]));
    }

    #[test]
    fn test_decode_unsupported_encoding() {
        let data = b"data";
        let result = decode_subscription_bytes(data.to_vec(), Some("lzma"));
        assert!(result.is_ok());
    }
}
