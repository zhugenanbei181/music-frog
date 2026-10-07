//! Preserve controller wire identity while keeping configuration decoding strict.
use crate::proxy::Proxy;
use serde::{Serialize, Serializer};

#[derive(Serialize)]
struct TaggedProxy<'a, T> {
    #[serde(rename = "type")]
    kind: &'a str,
    #[serde(flatten)]
    fields: &'a T,
}

impl Serialize for Proxy {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let kind = self.proxy_type();
        match self {
            Self::Observed(facts) => facts.serialize(serializer),
            Self::Shadowsocks(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Vmess(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Trojan(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Hysteria2(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::WireGuard(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Tuic(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Vless(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Http(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Socks5(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Snell(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Direct(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Reject(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Selector(fields)
            | Self::URLTest(fields)
            | Self::Fallback(fields)
            | Self::LoadBalance(fields)
            | Self::Relay(fields) => TaggedProxy { kind, fields }.serialize(serializer),
            Self::Unknown => {
                #[derive(Serialize)]
                struct UnknownKind {
                    #[serde(rename = "type")]
                    kind: &'static str,
                }
                UnknownKind { kind: "Unknown" }.serialize(serializer)
            }
        }
    }
}
