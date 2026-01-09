use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename = "multistatus", rename_all = "lowercase")]
pub struct MultiStatus {
    #[serde(rename = "response", default)]
    pub responses: Vec<Response>,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    pub href: String,
    #[serde(rename = "propstat")]
    pub propstats: Vec<PropStat>,
}

#[derive(Debug, Deserialize)]
pub struct PropStat {
    pub prop: Prop,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct Prop {
    #[serde(rename = "getetag")]
    pub etag: Option<String>,
    #[serde(rename = "getlastmodified")]
    pub last_modified: Option<String>,
    #[serde(rename = "getcontentlength")]
    pub content_length: Option<u64>,
    #[serde(rename = "resourcetype")]
    pub resource_type: ResourceType,
}

#[derive(Debug, Deserialize)]
pub struct ResourceType {
    pub collection: Option<Collection>,
}

#[derive(Debug, Deserialize)]
pub struct Collection {}

pub fn parse_multistatus(xml: &str) -> Result<Vec<crate::RemoteEntry>> {
    let ms: MultiStatus = quick_xml::de::from_str(xml)
        .map_err(|e| anyhow!("Failed to parse WebDAV XML: {}", e))?;

    let mut entries = Vec::new();
    for resp in ms.responses {
        // 提取成功的 propstat (通常是 HTTP/1.1 200 OK)
        if let Some(ok_stat) = resp.propstats.iter().find(|s| s.status.contains("200")) {
            let is_dir = ok_stat.prop.resource_type.collection.is_some();
            
            // 解析最后修改时间
            let last_modified = ok_stat.prop.last_modified.as_deref()
                .and_then(|t| DateTime::parse_from_rfc2822(t).ok())
                .map(|t| t.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            entries.push(crate::RemoteEntry {
                path: urlencoding::decode(&resp.href)?.into_owned(),
                etag: ok_stat.prop.etag.clone().unwrap_or_default().replace('"', ""),
                last_modified,
                is_dir,
                size: ok_stat.prop.content_length.unwrap_or(0),
            });
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PROPFIND_RESPONSE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
    <D:response>
        <D:href>/mihomo/</D:href>
        <D:propstat>
            <D:prop>
                <D:resourcetype><D:collection/></D:resourcetype>
                <D:getlastmodified>Mon, 01 Jan 2024 12:00:00 GMT</D:getlastmodified>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
        </D:propstat>
    </D:response>
    <D:response>
        <D:href>/mihomo/config.yaml</D:href>
        <D:propstat>
            <D:prop>
                <D:resourcetype/>
                <D:getetag>"abc123"</D:getetag>
                <D:getlastmodified>Mon, 01 Jan 2024 12:30:00 GMT</D:getlastmodified>
                <D:getcontentlength>1024</D:getcontentlength>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
        </D:propstat>
    </D:response>
    <D:response>
        <D:href>/mihomo/rules%2Fproxy.yaml</D:href>
        <D:propstat>
            <D:prop>
                <D:resourcetype/>
                <D:getetag>"def456"</D:getetag>
                <D:getcontentlength>512</D:getcontentlength>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
        </D:propstat>
    </D:response>
</D:multistatus>"#;

    #[test]
    fn test_parse_multistatus_basic() {
        let entries = parse_multistatus(SAMPLE_PROPFIND_RESPONSE).unwrap();
        
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_parse_directory_entry() {
        let entries = parse_multistatus(SAMPLE_PROPFIND_RESPONSE).unwrap();
        
        let dir = entries.iter().find(|e| e.path == "/mihomo/").unwrap();
        assert!(dir.is_dir);
    }

    #[test]
    fn test_parse_file_entry() {
        let entries = parse_multistatus(SAMPLE_PROPFIND_RESPONSE).unwrap();
        
        let file = entries.iter().find(|e| e.path == "/mihomo/config.yaml").unwrap();
        assert!(!file.is_dir);
        assert_eq!(file.etag, "abc123");
        assert_eq!(file.size, 1024);
    }

    #[test]
    fn test_parse_url_encoded_path() {
        let entries = parse_multistatus(SAMPLE_PROPFIND_RESPONSE).unwrap();
        
        // %2F should be decoded to /
        let file = entries.iter().find(|e| e.path.contains("rules/proxy")).unwrap();
        assert_eq!(file.path, "/mihomo/rules/proxy.yaml");
        assert_eq!(file.etag, "def456");
    }

    #[test]
    fn test_parse_empty_response() {
        let xml = r#"<?xml version="1.0"?><D:multistatus xmlns:D="DAV:"></D:multistatus>"#;
        let entries = parse_multistatus(xml).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_invalid_xml() {
        let result = parse_multistatus("not valid xml");
        assert!(result.is_err());
    }

    #[test]
    fn test_etag_quotes_stripped() {
        let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
    <D:response>
        <D:href>/test.yaml</D:href>
        <D:propstat>
            <D:prop>
                <D:resourcetype/>
                <D:getetag>"quoted-etag-value"</D:getetag>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
        </D:propstat>
    </D:response>
</D:multistatus>"#;
        
        let entries = parse_multistatus(xml).unwrap();
        assert_eq!(entries[0].etag, "quoted-etag-value");
    }
}
