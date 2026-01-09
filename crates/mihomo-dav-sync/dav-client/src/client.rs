use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::{header, Client, Method};
use url::Url;

use crate::{xml_parser, DavClient, RemoteEntry};

pub struct WebDavClient {
    client: Client,
    base_url: Url,
}

impl WebDavClient {
    pub fn new(url: &str, user: &str, pass: &str) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        let auth = format!("{}:{}", user, pass);
        let auth_header = format!("Basic {}", data_encoding::BASE64.encode(auth.as_bytes()));
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&auth_header)?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()?;

        let mut base_url = Url::parse(url)?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }

        Ok(Self { client, base_url })
    }

    fn full_url(&self, path: &str) -> Result<Url> {
        let path = path.trim_start_matches('/');
        self.base_url.join(path).map_err(|e| anyhow!("Invalid path: {}", e))
    }
}

#[async_trait]
impl DavClient for WebDavClient {
    async fn list(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        let url = self.full_url(path)?;
        let resp = self.client
            .request(Method::from_bytes(b"PROPFIND")?, url)
            .header("Depth", "1")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("PROPFIND failed: {}", resp.status()));
        }

        let body = resp.text().await?;
        xml_parser::parse_multistatus(&body)
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>> {
        let url = self.full_url(path)?;
        let resp = self.client.get(url).send().await?;
        
        if !resp.status().is_success() {
            return Err(anyhow!("GET failed: {}", resp.status()));
        }
        
        Ok(resp.bytes().await?.to_vec())
    }

    async fn put(&self, path: &str, data: &[u8], if_match: Option<&str>) -> Result<String> {
        let url = self.full_url(path)?;
        let mut req = self.client.put(url).body(data.to_owned());
        
        if let Some(etag) = if_match {
            req = req.header(header::IF_MATCH, format!("\"{}\"", etag));
        }

        let resp = req.send().await?;
        
        if !resp.status().is_success() {
            return Err(anyhow!("PUT failed: {}", resp.status()));
        }

        // 尝试从响应头提取新 ETag
        let etag = resp.headers()
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.replace('"', ""))
            .unwrap_or_default();

        Ok(etag)
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let url = self.full_url(path)?;
        let resp = self.client.delete(url).send().await?;
        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(anyhow!("DELETE failed: {}", resp.status()));
        }
        Ok(())
    }

    async fn move_item(&self, from: &str, to: &str) -> Result<()> {
        let from_url = self.full_url(from)?;
        let to_url = self.full_url(to)?;
        let resp = self.client
            .request(Method::from_bytes(b"MOVE")?, from_url)
            .header("Destination", to_url.to_string())
            .header("Overwrite", "T")
            .send()
            .await?;
        
        if !resp.status().is_success() {
            return Err(anyhow!("MOVE failed: {}", resp.status()));
        }
        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<()> {
        let url = self.full_url(path)?;
        let resp = self.client
            .request(Method::from_bytes(b"MKCOL")?, url)
            .send()
            .await?;
        
        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::METHOD_NOT_ALLOWED {
            // 405 Method Not Allowed 通常意味着目录已存在
            return Err(anyhow!("MKCOL failed: {}", resp.status()));
        }
        Ok(())
    }
}
