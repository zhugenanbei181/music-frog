use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ProxyNodeItem {
    pub name: String,
    pub server: String,
    pub port: u16,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cipher: Option<String>,
    #[serde(default)]
    pub tls: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileFormat {
    ClashYaml,
    ShadowrocketUriList,
    RawJson,
    Base64Subscription,
}

pub struct ProfileConverter;

#[derive(Serialize, Deserialize)]
struct ClashProfile {
    #[serde(default)]
    proxies: Vec<ProxyNodeItem>,
}

impl ProfileConverter {
    pub fn parse_nodes(input: &str, format: ProfileFormat) -> Result<Vec<ProxyNodeItem>> {
        match format {
            ProfileFormat::ClashYaml => {
                let profile: ClashProfile = serde_yaml_ng::from_str(input)
                    .map_err(|e| anyhow!("Failed to parse YAML: {}", e))?;
                Ok(profile.proxies)
            }
            ProfileFormat::RawJson => {
                let nodes: Vec<ProxyNodeItem> = serde_json::from_str(input)
                    .map_err(|e| anyhow!("Failed to parse JSON: {}", e))?;
                Ok(nodes)
            }
            ProfileFormat::ShadowrocketUriList => {
                let mut nodes = Vec::new();
                for line in input.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Ok(node) = Self::parse_uri(line) {
                        nodes.push(node);
                    }
                }
                Ok(nodes)
            }
            ProfileFormat::Base64Subscription => {
                let decoded = STANDARD
                    .decode(input.trim())
                    .map_err(|e| anyhow!("Failed to decode Base64: {}", e))?;
                let text = String::from_utf8(decoded)
                    .map_err(|e| anyhow!("Invalid UTF-8 in Base64: {}", e))?;
                Self::parse_nodes(&text, ProfileFormat::ShadowrocketUriList)
            }
        }
    }

    pub fn export_nodes(nodes: &[ProxyNodeItem], target_format: ProfileFormat) -> Result<String> {
        match target_format {
            ProfileFormat::ClashYaml => {
                let profile = ClashProfile {
                    proxies: nodes.to_vec(),
                };
                serde_yaml_ng::to_string(&profile)
                    .map_err(|e| anyhow!("Failed to export YAML: {}", e))
            }
            ProfileFormat::RawJson => serde_json::to_string_pretty(nodes)
                .map_err(|e| anyhow!("Failed to export JSON: {}", e)),
            ProfileFormat::ShadowrocketUriList => {
                let mut out = String::new();
                for node in nodes {
                    out.push_str(&Self::export_uri(node)?);
                    out.push('\n');
                }
                Ok(out)
            }
            ProfileFormat::Base64Subscription => {
                let uri_list = Self::export_nodes(nodes, ProfileFormat::ShadowrocketUriList)?;
                Ok(STANDARD.encode(uri_list))
            }
        }
    }

    pub fn convert(input: &str, from_fmt: ProfileFormat, to_fmt: ProfileFormat) -> Result<String> {
        let nodes = Self::parse_nodes(input, from_fmt)?;
        Self::export_nodes(&nodes, to_fmt)
    }

    fn parse_uri(uri: &str) -> Result<ProxyNodeItem> {
        let parsed = Url::parse(uri)?;
        let scheme = parsed.scheme();

        match scheme {
            "ss" => {
                let username = parsed.username();
                let (cipher, password) = if username.contains(':') {
                    let (c, p) = username.split_once(':').unwrap();
                    let decoded_c = urlencoding::decode(c).unwrap_or_default().to_string();
                    let decoded_p = urlencoding::decode(p).unwrap_or_default().to_string();
                    (Some(decoded_c), Some(decoded_p))
                } else if !username.is_empty() {
                    let decoded = STANDARD.decode(username).unwrap_or_default();
                    let decoded_str = String::from_utf8(decoded).unwrap_or_default();
                    if let Some((c, p)) = decoded_str.split_once(':') {
                        (Some(c.to_string()), Some(p.to_string()))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                let name = parsed.fragment().unwrap_or("").to_string();
                let server = parsed.host_str().unwrap_or("").to_string();
                let port = parsed.port().unwrap_or(80);

                Ok(ProxyNodeItem {
                    name: if name.is_empty() {
                        "ss-node".to_string()
                    } else {
                        urlencoding::decode(&name).unwrap_or_default().to_string()
                    },
                    server,
                    port,
                    node_type: "ss".to_string(),
                    password,
                    uuid: None,
                    cipher,
                    tls: false,
                })
            }
            "vmess" => {
                let host = parsed.host_str().unwrap_or("");
                let decoded = STANDARD
                    .decode(host)
                    .map_err(|e| anyhow!("Invalid vmess base64: {}", e))?;
                let json_str =
                    String::from_utf8(decoded).map_err(|e| anyhow!("Invalid vmess utf8: {}", e))?;

                #[derive(Deserialize)]
                struct VmessNode {
                    #[serde(default)]
                    ps: String,
                    #[serde(default)]
                    add: String,
                    #[serde(default)]
                    port: u16,
                    #[serde(default)]
                    id: String,
                    #[serde(default)]
                    tls: String,
                }

                let vnode: VmessNode = serde_json::from_str(&json_str)?;
                Ok(ProxyNodeItem {
                    name: vnode.ps,
                    server: vnode.add,
                    port: vnode.port,
                    node_type: "vmess".to_string(),
                    password: None,
                    uuid: Some(vnode.id),
                    cipher: None,
                    tls: vnode.tls == "tls",
                })
            }
            "trojan" => {
                let password = Some(
                    urlencoding::decode(parsed.username())
                        .unwrap_or_default()
                        .to_string(),
                );
                let server = parsed.host_str().unwrap_or("").to_string();
                let port = parsed.port().unwrap_or(443);
                let name = parsed.fragment().unwrap_or("").to_string();

                Ok(ProxyNodeItem {
                    name: if name.is_empty() {
                        "trojan-node".to_string()
                    } else {
                        urlencoding::decode(&name).unwrap_or_default().to_string()
                    },
                    server,
                    port,
                    node_type: "trojan".to_string(),
                    password,
                    uuid: None,
                    cipher: None,
                    tls: true,
                })
            }
            _ => Err(anyhow!("Unsupported scheme: {}", scheme)),
        }
    }

    fn export_uri(node: &ProxyNodeItem) -> Result<String> {
        match node.node_type.as_str() {
            "ss" => {
                let user_pass = format!(
                    "{}:{}",
                    node.cipher.as_deref().unwrap_or("none"),
                    node.password.as_deref().unwrap_or("")
                );
                let encoded_user = STANDARD.encode(user_pass);
                Ok(format!(
                    "ss://{}@{}:{}#{}",
                    encoded_user,
                    node.server,
                    node.port,
                    urlencoding::encode(&node.name)
                ))
            }
            "vmess" => {
                #[derive(Serialize)]
                struct VmessNode {
                    v: String,
                    ps: String,
                    add: String,
                    port: u16,
                    id: String,
                    tls: String,
                }
                let vnode = VmessNode {
                    v: "2".to_string(),
                    ps: node.name.clone(),
                    add: node.server.clone(),
                    port: node.port,
                    id: node.uuid.clone().unwrap_or_default(),
                    tls: if node.tls {
                        "tls".to_string()
                    } else {
                        "".to_string()
                    },
                };
                let json_str = serde_json::to_string(&vnode)?;
                let encoded = STANDARD.encode(json_str);
                Ok(format!("vmess://{}", encoded))
            }
            "trojan" => {
                let pwd = urlencoding::encode(node.password.as_deref().unwrap_or(""));
                Ok(format!(
                    "trojan://{}@{}:{}#{}",
                    pwd,
                    node.server,
                    node.port,
                    urlencoding::encode(&node.name)
                ))
            }
            _ => Err(anyhow!(
                "Unsupported node type for URI export: {}",
                node.node_type
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
proxies:
  - name: "ss-node"
    type: ss
    server: 1.2.3.4
    port: 8388
    cipher: aes-256-gcm
    password: "password123"
  - name: "vmess-node"
    type: vmess
    server: 1.1.1.1
    port: 443
    uuid: "b831381d-6324-4d53-ad4f-8cda48b30811"
    tls: true
"#;
        let nodes = ProfileConverter::parse_nodes(yaml, ProfileFormat::ClashYaml).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "ss-node");
        assert_eq!(nodes[0].node_type, "ss");
        assert_eq!(nodes[1].name, "vmess-node");
        assert_eq!(nodes[1].node_type, "vmess");
        assert!(nodes[1].tls);
    }

    #[test]
    fn test_roundtrip_json() {
        let node = ProxyNodeItem {
            name: "test-trojan".to_string(),
            server: "example.com".to_string(),
            port: 443,
            node_type: "trojan".to_string(),
            password: Some("secret".to_string()),
            uuid: None,
            cipher: None,
            tls: true,
        };

        let json =
            ProfileConverter::export_nodes(std::slice::from_ref(&node), ProfileFormat::RawJson)
                .unwrap();
        let parsed = ProfileConverter::parse_nodes(&json, ProfileFormat::RawJson).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], node);
    }

    #[test]
    fn test_roundtrip_uri_base64() {
        let node = ProxyNodeItem {
            name: "vmess-test".to_string(),
            server: "v.example.com".to_string(),
            port: 443,
            node_type: "vmess".to_string(),
            password: None,
            uuid: Some("uuid-1234".to_string()),
            cipher: None,
            tls: true,
        };

        let b64 = ProfileConverter::export_nodes(
            std::slice::from_ref(&node),
            ProfileFormat::Base64Subscription,
        )
        .unwrap();
        let parsed =
            ProfileConverter::parse_nodes(&b64, ProfileFormat::Base64Subscription).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], node);
    }

    #[test]
    fn test_convert_format() {
        let yaml = r#"
proxies:
  - name: "ss1"
    type: ss
    server: 1.1.1.1
    port: 1080
    cipher: dummy
    password: pass
"#;
        let json =
            ProfileConverter::convert(yaml, ProfileFormat::ClashYaml, ProfileFormat::RawJson)
                .unwrap();
        assert!(json.contains("ss1"));
        assert!(json.contains("1.1.1.1"));

        let nodes = ProfileConverter::parse_nodes(&json, ProfileFormat::RawJson).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "ss1");
    }
}
