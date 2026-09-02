//! Web API shim implementations for the sandboxed script runtime.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;

use super::{
    Base64Shim, CryptoSubtleShim, FetchPermissionShim, HeadersShim, PluginPermission, ScriptError,
    UrlShim,
};

impl HeadersShim {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
    pub fn from_pairs<I: IntoIterator<Item = (K, V)>, K: Into<String>, V: Into<String>>(
        pairs: I,
    ) -> Self {
        Self {
            entries: pairs
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }
    pub fn append(&mut self, name: &str, value: &str) {
        self.entries.push((name.to_string(), value.to_string()));
    }
    pub fn set(&mut self, name: &str, value: &str) {
        self.entries.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
        self.entries.push((name.to_string(), value.to_string()));
    }
    pub fn get(&self, name: &str) -> Option<String> {
        let m: Vec<&str> = self
            .entries
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect();
        (!m.is_empty()).then(|| m.join(", "))
    }
    pub fn get_all(&self, name: &str) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
            .collect()
    }
    pub fn has(&self, name: &str) -> bool {
        self.entries
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case(name))
    }
    pub fn delete(&mut self, name: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
        len != self.entries.len()
    }
    pub fn entries(&self) -> &[(String, String)] {
        &self.entries
    }
    pub fn keys(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut keys = Vec::new();
        for (k, _) in &self.entries {
            let lower = k.to_ascii_lowercase();
            if seen.insert(lower.clone()) {
                keys.push(lower);
            }
        }
        keys
    }
    pub fn values(&self) -> Vec<String> {
        self.entries.iter().map(|(_, v)| v.clone()).collect()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Serialize for UrlShim {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.inner.as_str())
    }
}

impl<'de> Deserialize<'de> for UrlShim {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for UrlShim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.as_str())
    }
}

impl UrlShim {
    pub fn parse(input: &str) -> Result<Self, ScriptError> {
        url::Url::parse(input)
            .map(|inner| Self { inner })
            .map_err(|e| ScriptError::Runtime(format!("Failed to parse URL `{input}`: {e}")))
    }
    pub fn new(input: &str) -> Result<Self, ScriptError> {
        Self::parse(input)
    }
    pub fn href(&self) -> &str {
        self.inner.as_str()
    }
    pub fn set_href(&mut self, href: &str) -> Result<(), ScriptError> {
        self.inner = url::Url::parse(href)
            .map_err(|e| ScriptError::Runtime(format!("Invalid URL href `{href}`: {e}")))?;
        Ok(())
    }
    pub fn protocol(&self) -> String {
        format!("{}:", self.inner.scheme())
    }
    pub fn set_protocol(&mut self, protocol: &str) -> Result<(), ScriptError> {
        self.inner
            .set_scheme(protocol.trim_end_matches(':'))
            .map_err(|_| ScriptError::Runtime(format!("Invalid scheme `{protocol}`")))?;
        Ok(())
    }
    pub fn username(&self) -> &str {
        self.inner.username()
    }
    pub fn set_username(&mut self, username: &str) -> Result<(), ScriptError> {
        self.inner.set_username(username).map_err(|_| {
            ScriptError::Runtime(format!("Cannot set username on `{}`", self.inner))
        })?;
        Ok(())
    }
    pub fn password(&self) -> Option<&str> {
        self.inner.password()
    }
    pub fn set_password(&mut self, password: Option<&str>) -> Result<(), ScriptError> {
        self.inner.set_password(password).map_err(|_| {
            ScriptError::Runtime(format!("Cannot set password on `{}`", self.inner))
        })?;
        Ok(())
    }
    pub fn host(&self) -> String {
        match (self.inner.host_str(), self.inner.port()) {
            (Some(h), Some(p)) => format!("{h}:{p}"),
            (Some(h), None) => h.to_string(),
            (None, _) => String::new(),
        }
    }
    pub fn set_host(&mut self, host_str: &str) -> Result<(), ScriptError> {
        if let Some((h, p)) = host_str.rsplit_once(':') {
            self.inner
                .set_host(Some(h))
                .map_err(|e| ScriptError::Runtime(format!("Invalid host: {e:?}")))?;
            let port = p
                .parse::<u16>()
                .map_err(|e| ScriptError::Runtime(format!("Invalid port `{p}`: {e}")))?;
            self.inner
                .set_port(Some(port))
                .map_err(|_| ScriptError::Runtime("Cannot set port".to_string()))?;
        } else {
            self.inner
                .set_host(Some(host_str))
                .map_err(|e| ScriptError::Runtime(format!("Invalid host: {e:?}")))?;
            let _ = self.inner.set_port(None);
        }
        Ok(())
    }
    pub fn hostname(&self) -> &str {
        self.inner.host_str().unwrap_or("")
    }
    pub fn set_hostname(&mut self, hostname: &str) -> Result<(), ScriptError> {
        self.inner
            .set_host(Some(hostname))
            .map_err(|e| ScriptError::Runtime(format!("Invalid hostname: {e:?}")))?;
        Ok(())
    }
    pub fn port(&self) -> Option<u16> {
        self.inner.port()
    }
    pub fn port_str(&self) -> String {
        self.inner
            .port()
            .map_or_else(String::new, |p| p.to_string())
    }
    pub fn set_port(&mut self, port: Option<u16>) -> Result<(), ScriptError> {
        self.inner
            .set_port(port)
            .map_err(|_| ScriptError::Runtime("Cannot set port".to_string()))?;
        Ok(())
    }
    pub fn pathname(&self) -> &str {
        self.inner.path()
    }
    pub fn set_pathname(&mut self, path: &str) {
        self.inner.set_path(path);
    }
    pub fn search(&self) -> String {
        self.inner
            .query()
            .map_or_else(String::new, |q| format!("?{q}"))
    }
    pub fn set_search(&mut self, search: &str) {
        let q = search.strip_prefix('?').unwrap_or(search);
        self.inner
            .set_query(if q.is_empty() { None } else { Some(q) });
    }
    pub fn hash(&self) -> String {
        self.inner
            .fragment()
            .map_or_else(String::new, |f| format!("#{f}"))
    }
    pub fn set_hash(&mut self, hash: &str) {
        let f = hash.strip_prefix('#').unwrap_or(hash);
        self.inner
            .set_fragment(if f.is_empty() { None } else { Some(f) });
    }
    pub fn origin(&self) -> String {
        self.inner.origin().ascii_serialization()
    }
    pub fn get_search_param(&self, key: &str) -> Option<String> {
        self.inner
            .query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    }
    pub fn get_all_search_params(&self, key: &str) -> Vec<String> {
        self.inner
            .query_pairs()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
            .collect()
    }
    pub fn has_search_param(&self, key: &str) -> bool {
        self.inner.query_pairs().any(|(k, _)| k == key)
    }
    pub fn set_search_param(&mut self, key: &str, value: &str) {
        let mut pairs: Vec<(String, String)> = self
            .inner
            .query_pairs()
            .filter(|(k, _)| k != key)
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        pairs.push((key.to_string(), value.to_string()));
        self.update_query_pairs(pairs);
    }
    pub fn append_search_param(&mut self, key: &str, value: &str) {
        let mut pairs: Vec<(String, String)> = self
            .inner
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        pairs.push((key.to_string(), value.to_string()));
        self.update_query_pairs(pairs);
    }
    pub fn delete_search_param(&mut self, key: &str) {
        let pairs: Vec<(String, String)> = self
            .inner
            .query_pairs()
            .filter(|(k, _)| k != key)
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        self.update_query_pairs(pairs);
    }
    fn update_query_pairs(&mut self, pairs: Vec<(String, String)>) {
        if pairs.is_empty() {
            self.inner.set_query(None);
        } else {
            let mut ser = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in pairs {
                ser.append_pair(&k, &v);
            }
            self.inner.set_query(Some(&ser.finish()));
        }
    }
    pub fn search_params(&self) -> Vec<(String, String)> {
        self.inner
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }
}

impl CryptoSubtleShim {
    pub fn digest(algorithm: &str, data: &[u8]) -> Result<Vec<u8>, ScriptError> {
        let algo = algorithm.trim().to_ascii_uppercase().replace('-', "");
        match algo.as_str() {
            "SHA1" => Ok(
                ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, data)
                    .as_ref()
                    .to_vec(),
            ),
            "SHA256" => Ok(ring::digest::digest(&ring::digest::SHA256, data)
                .as_ref()
                .to_vec()),
            "SHA384" => Ok(ring::digest::digest(&ring::digest::SHA384, data)
                .as_ref()
                .to_vec()),
            "SHA512" => Ok(ring::digest::digest(&ring::digest::SHA512, data)
                .as_ref()
                .to_vec()),
            _ => Err(ScriptError::Runtime(format!(
                "Unsupported digest algorithm `{algorithm}`"
            ))),
        }
    }

    pub fn digest_hex(algorithm: &str, data: &[u8]) -> Result<String, ScriptError> {
        Ok(Self::digest(algorithm, data)?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect())
    }

    pub fn digest_base64(algorithm: &str, data: &[u8]) -> Result<String, ScriptError> {
        use base64::Engine;
        Ok(base64::engine::general_purpose::STANDARD.encode(Self::digest(algorithm, data)?))
    }

    pub fn hmac_sign(algorithm: &str, key: &[u8], data: &[u8]) -> Result<Vec<u8>, ScriptError> {
        let algo = match algorithm
            .trim()
            .to_ascii_uppercase()
            .replace('-', "")
            .as_str()
        {
            "SHA1" | "HMACSHA1" => ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
            "SHA256" | "HMACSHA256" => ring::hmac::HMAC_SHA256,
            "SHA384" | "HMACSHA384" => ring::hmac::HMAC_SHA384,
            "SHA512" | "HMACSHA512" => ring::hmac::HMAC_SHA512,
            _ => {
                return Err(ScriptError::Runtime(format!(
                    "Unsupported HMAC algorithm `{algorithm}`"
                )));
            }
        };
        let s_key = ring::hmac::Key::new(algo, key);
        Ok(ring::hmac::sign(&s_key, data).as_ref().to_vec())
    }

    pub fn hmac_verify(
        algorithm: &str,
        key: &[u8],
        signature: &[u8],
        data: &[u8],
    ) -> Result<bool, ScriptError> {
        let algo = match algorithm
            .trim()
            .to_ascii_uppercase()
            .replace('-', "")
            .as_str()
        {
            "SHA1" | "HMACSHA1" => ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
            "SHA256" | "HMACSHA256" => ring::hmac::HMAC_SHA256,
            "SHA384" | "HMACSHA384" => ring::hmac::HMAC_SHA384,
            "SHA512" | "HMACSHA512" => ring::hmac::HMAC_SHA512,
            _ => {
                return Err(ScriptError::Runtime(format!(
                    "Unsupported HMAC algorithm `{algorithm}`"
                )));
            }
        };
        let s_key = ring::hmac::Key::new(algo, key);
        Ok(ring::hmac::verify(&s_key, data, signature).is_ok())
    }

    pub fn aes_gcm_encrypt(
        key: &[u8],
        nonce: &[u8],
        plaintext: &[u8],
        aad: Option<&[u8]>,
    ) -> Result<Vec<u8>, ScriptError> {
        use ring::aead::{AES_128_GCM, AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
        let algo = match key.len() {
            16 => &AES_128_GCM,
            32 => &AES_256_GCM,
            len => return Err(ScriptError::Runtime(format!("Invalid AES key len ({len})"))),
        };
        let k = LessSafeKey::new(
            UnboundKey::new(algo, key)
                .map_err(|_| ScriptError::Runtime("Invalid AES key".to_string()))?,
        );
        let n = Nonce::try_assume_unique_for_key(nonce)
            .map_err(|_| ScriptError::Runtime("Nonce must be 12 bytes".to_string()))?;
        let mut in_out = plaintext.to_vec();
        k.seal_in_place_append_tag(n, Aad::from(aad.unwrap_or(b"")), &mut in_out)
            .map_err(|e| ScriptError::Runtime(format!("AES encryption failed: {e:?}")))?;
        Ok(in_out)
    }

    pub fn aes_gcm_decrypt(
        key: &[u8],
        nonce: &[u8],
        ciphertext: &[u8],
        aad: Option<&[u8]>,
    ) -> Result<Vec<u8>, ScriptError> {
        use ring::aead::{AES_128_GCM, AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
        let algo = match key.len() {
            16 => &AES_128_GCM,
            32 => &AES_256_GCM,
            len => return Err(ScriptError::Runtime(format!("Invalid AES key len ({len})"))),
        };
        let k = LessSafeKey::new(
            UnboundKey::new(algo, key)
                .map_err(|_| ScriptError::Runtime("Invalid AES key".to_string()))?,
        );
        let n = Nonce::try_assume_unique_for_key(nonce)
            .map_err(|_| ScriptError::Runtime("Nonce must be 12 bytes".to_string()))?;
        let mut in_out = ciphertext.to_vec();
        let decrypted = k
            .open_in_place(n, Aad::from(aad.unwrap_or(b"")), &mut in_out)
            .map_err(|_| ScriptError::Runtime("AES decryption failed".to_string()))?;
        Ok(decrypted.to_vec())
    }

    pub fn get_random_values(buf: &mut [u8]) -> Result<(), ScriptError> {
        use ring::rand::SecureRandom;
        ring::rand::SystemRandom::new()
            .fill(buf)
            .map_err(|_| ScriptError::Runtime("Failed to generate random bytes".to_string()))
    }

    pub fn random_bytes(len: usize) -> Result<Vec<u8>, ScriptError> {
        let mut buf = vec![0u8; len];
        Self::get_random_values(&mut buf)?;
        Ok(buf)
    }

    pub fn timing_safe_equal(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut diff = 0u8;
        for (&x, &y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }
}

impl Base64Shim {
    pub fn encode(data: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(data)
    }

    pub fn decode(encoded: &str) -> Result<Vec<u8>, ScriptError> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|e| ScriptError::Runtime(format!("Base64 decode error: {e}")))
    }

    pub fn encode_url_safe(data: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
    }

    pub fn decode_url_safe(encoded: &str) -> Result<Vec<u8>, ScriptError> {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(encoded.trim()))
            .map_err(|e| ScriptError::Runtime(format!("URL-safe Base64 decode error: {e}")))
    }
}

impl FetchPermissionShim {
    pub fn check_permission(
        permissions: &HashSet<PluginPermission>,
        target_url: &str,
        allowed_domains: Option<&[&str]>,
    ) -> Result<(), ScriptError> {
        if !permissions.contains(&PluginPermission::NetworkAccess) {
            return Err(ScriptError::Runtime(
                "Network access permission denied: plugin manifest lacks `network_access` permission"
                    .to_string(),
            ));
        }

        if let Some(whitelist) = allowed_domains {
            if !whitelist.is_empty() {
                let parsed = url::Url::parse(target_url)
                    .map_err(|e| ScriptError::Runtime(format!("Invalid fetch URL: {e}")))?;
                let host = parsed.host_str().unwrap_or("");
                let allowed = whitelist.iter().any(|domain| {
                    host.eq_ignore_ascii_case(domain) || host.ends_with(&format!(".{domain}"))
                });
                if !allowed {
                    return Err(ScriptError::Runtime(format!(
                        "Fetch target `{host}` is not in plugin domain allowlist"
                    )));
                }
            }
        }
        Ok(())
    }
}
