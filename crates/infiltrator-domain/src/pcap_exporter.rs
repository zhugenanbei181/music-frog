//! PCAP packet capture serialization and HTTP/URL rewrite engine.

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Standard PCAP global file header (24 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PcapHeader {
    pub magic: u32,
    pub version_major: u16,
    pub version_minor: u16,
    pub thiszone: i32,
    pub sigfigs: u32,
    pub snaplen: u32,
    pub link_type: u32,
}

impl PcapHeader {
    pub const MAGIC_MICROS: u32 = 0xa1b2c3d4;
    pub const MAGIC_NANOS: u32 = 0xa1b23c4d;
    pub const DEFAULT_SNAPLEN: u32 = 65535;
    pub const LINKTYPE_ETHERNET: u32 = 1;
    pub const LINKTYPE_RAW: u32 = 12;
    pub const LINKTYPE_IPV4: u32 = 228;

    pub fn new(snaplen: u32, link_type: u32) -> Self {
        Self {
            magic: Self::MAGIC_MICROS,
            version_major: 2,
            version_minor: 4,
            thiszone: 0,
            sigfigs: 0,
            snaplen,
            link_type,
        }
    }

    #[must_use]
    pub fn with_nanoseconds(mut self) -> Self {
        self.magic = Self::MAGIC_NANOS;
        self
    }

    pub fn is_nanosecond_precision(&self) -> bool {
        self.magic == Self::MAGIC_NANOS
    }

    pub fn to_bytes(&self) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[0..4].copy_from_slice(&self.magic.to_le_bytes());
        b[4..6].copy_from_slice(&self.version_major.to_le_bytes());
        b[6..8].copy_from_slice(&self.version_minor.to_le_bytes());
        b[8..12].copy_from_slice(&self.thiszone.to_le_bytes());
        b[12..16].copy_from_slice(&self.sigfigs.to_le_bytes());
        b[16..20].copy_from_slice(&self.snaplen.to_le_bytes());
        b[20..24].copy_from_slice(&self.link_type.to_le_bytes());
        b
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        if b.len() < 24 {
            bail!("PCAP header requires 24 bytes, got {}", b.len());
        }
        let magic = u32::from_le_bytes(b[0..4].try_into().unwrap());
        if magic != Self::MAGIC_MICROS && magic != Self::MAGIC_NANOS {
            bail!("Invalid PCAP magic: 0x{magic:08x}");
        }
        Ok(Self {
            magic,
            version_major: u16::from_le_bytes(b[4..6].try_into().unwrap()),
            version_minor: u16::from_le_bytes(b[6..8].try_into().unwrap()),
            thiszone: i32::from_le_bytes(b[8..12].try_into().unwrap()),
            sigfigs: u32::from_le_bytes(b[12..16].try_into().unwrap()),
            snaplen: u32::from_le_bytes(b[16..20].try_into().unwrap()),
            link_type: u32::from_le_bytes(b[20..24].try_into().unwrap()),
        })
    }
}

impl Default for PcapHeader {
    fn default() -> Self {
        Self::new(Self::DEFAULT_SNAPLEN, Self::LINKTYPE_ETHERNET)
    }
}

/// Standard PCAP record packet header (16 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PcapRecordHeader {
    pub ts_sec: u32,
    pub ts_subsec: u32,
    pub caplen: u32,
    pub orig_len: u32,
}

impl PcapRecordHeader {
    pub fn new(ts_sec: u32, ts_subsec: u32, caplen: u32, orig_len: u32) -> Self {
        Self { ts_sec, ts_subsec, caplen, orig_len }
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[0..4].copy_from_slice(&self.ts_sec.to_le_bytes());
        b[4..8].copy_from_slice(&self.ts_subsec.to_le_bytes());
        b[8..12].copy_from_slice(&self.caplen.to_le_bytes());
        b[12..16].copy_from_slice(&self.orig_len.to_le_bytes());
        b
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        if b.len() < 16 {
            bail!("PCAP record header requires 16 bytes, got {}", b.len());
        }
        Ok(Self {
            ts_sec: u32::from_le_bytes(b[0..4].try_into().unwrap()),
            ts_subsec: u32::from_le_bytes(b[4..8].try_into().unwrap()),
            caplen: u32::from_le_bytes(b[8..12].try_into().unwrap()),
            orig_len: u32::from_le_bytes(b[12..16].try_into().unwrap()),
        })
    }
}

/// Parsed packet payload with its record header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedPacket {
    pub header: PcapRecordHeader,
    pub data: Vec<u8>,
}

/// PCAP packet capture exporter.
#[derive(Debug, Clone)]
pub struct PcapExporter {
    header: PcapHeader,
    buffer: Vec<u8>,
    packet_count: usize,
    total_bytes_captured: usize,
}

impl PcapExporter {
    pub fn new(header: PcapHeader) -> Self {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&header.to_bytes());
        Self { header, buffer, packet_count: 0, total_bytes_captured: 0 }
    }

    pub fn write_header() -> Vec<u8> {
        PcapHeader::default().to_bytes().to_vec()
    }

    pub fn write_packet(ts_unix_secs: u64, ts_micros: u32, data: &[u8]) -> Vec<u8> {
        let ts_sec = (ts_unix_secs & 0xffff_ffff) as u32;
        let caplen = (data.len().min(PcapHeader::DEFAULT_SNAPLEN as usize)) as u32;
        let record = PcapRecordHeader::new(ts_sec, ts_micros, caplen, data.len() as u32);
        let mut out = Vec::with_capacity(16 + caplen as usize);
        out.extend_from_slice(&record.to_bytes());
        out.extend_from_slice(&data[..caplen as usize]);
        out
    }

    pub fn append_packet(&mut self, ts_unix_secs: u64, ts_subsec: u32, data: &[u8]) -> usize {
        self.append_packet_with_wire_len(ts_unix_secs, ts_subsec, data, data.len() as u32)
    }

    pub fn append_packet_with_wire_len(
        &mut self,
        ts_unix_secs: u64,
        ts_subsec: u32,
        data: &[u8],
        wire_len: u32,
    ) -> usize {
        let ts_sec = (ts_unix_secs & 0xffff_ffff) as u32;
        let caplen = (data.len().min(self.header.snaplen as usize)) as u32;
        let record = PcapRecordHeader::new(ts_sec, ts_subsec, caplen, wire_len);
        self.buffer.extend_from_slice(&record.to_bytes());
        self.buffer.extend_from_slice(&data[..caplen as usize]);
        self.packet_count += 1;
        self.total_bytes_captured += caplen as usize;
        16 + caplen as usize
    }

    pub fn as_bytes(&self) -> &[u8] { &self.buffer }
    pub fn into_bytes(self) -> Vec<u8> { self.buffer }
    pub fn header(&self) -> &PcapHeader { &self.header }
    pub fn packet_count(&self) -> usize { self.packet_count }
    pub fn total_bytes_captured(&self) -> usize { self.total_bytes_captured }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.buffer.extend_from_slice(&self.header.to_bytes());
        self.packet_count = 0;
        self.total_bytes_captured = 0;
    }

    pub fn parse_packets(bytes: &[u8]) -> Result<(PcapHeader, Vec<CapturedPacket>)> {
        if bytes.len() < 24 {
            bail!("Buffer too short for PCAP header: {} bytes", bytes.len());
        }
        let header = PcapHeader::from_bytes(&bytes[0..24])?;
        let mut packets = Vec::new();
        let mut offset = 24;
        while offset + 16 <= bytes.len() {
            let rec_hdr = PcapRecordHeader::from_bytes(&bytes[offset..offset + 16])?;
            offset += 16;
            let caplen = rec_hdr.caplen as usize;
            if offset + caplen > bytes.len() {
                bail!("Truncated packet data at offset {offset}");
            }
            let payload = bytes[offset..offset + caplen].to_vec();
            offset += caplen;
            packets.push(CapturedPacket { header: rec_hdr, data: payload });
        }
        Ok((header, packets))
    }
}

impl Default for PcapExporter {
    fn default() -> Self {
        Self::new(PcapHeader::default())
    }
}

pub fn write_header() -> Vec<u8> {
    PcapExporter::write_header()
}

pub fn write_packet(ts_unix_secs: u64, ts_micros: u32, data: &[u8]) -> Vec<u8> {
    PcapExporter::write_packet(ts_unix_secs, ts_micros, data)
}

/// Outcome of evaluating a URL rewrite rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RewriteOutcome {
    Redirect302(String),
    Redirect307(String),
    DirectRewrite(String),
}

impl RewriteOutcome {
    pub fn destination(&self) -> &str {
        match self {
            Self::Redirect302(d) | Self::Redirect307(d) | Self::DirectRewrite(d) => d,
        }
    }

    pub fn is_redirect(&self) -> bool {
        matches!(self, Self::Redirect302(_) | Self::Redirect307(_))
    }

    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Redirect302(_) => Some(302),
            Self::Redirect307(_) => Some(307),
            Self::DirectRewrite(_) => None,
        }
    }
}

/// Individual URL rewrite rule matching regular expressions.
#[derive(Debug, Clone)]
pub struct RewriteRule {
    pub pattern: Regex,
    pub replacement: String,
    pub status_code: Option<u16>,
    pub redirect: bool,
}

impl RewriteRule {
    pub fn new(pattern: &str, replacement: &str, redirect: bool, status_code: Option<u16>) -> Result<Self> {
        let reg = Regex::new(pattern).with_context(|| format!("Invalid regex: '{pattern}'"))?;
        Ok(Self { pattern: reg, replacement: replacement.to_string(), status_code, redirect })
    }

    pub fn redirect_302(pattern: &str, replacement: &str) -> Result<Self> {
        Self::new(pattern, replacement, true, Some(302))
    }

    pub fn redirect_307(pattern: &str, replacement: &str) -> Result<Self> {
        Self::new(pattern, replacement, true, Some(307))
    }

    pub fn direct(pattern: &str, replacement: &str) -> Result<Self> {
        Self::new(pattern, replacement, false, None)
    }

    pub fn apply(&self, url: &str) -> Option<RewriteOutcome> {
        if !self.pattern.is_match(url) {
            return None;
        }
        let rewritten = self.pattern.replace_all(url, &self.replacement).to_string();
        if self.redirect {
            if self.status_code == Some(307) {
                Some(RewriteOutcome::Redirect307(rewritten))
            } else {
                Some(RewriteOutcome::Redirect302(rewritten))
            }
        } else {
            Some(RewriteOutcome::DirectRewrite(rewritten))
        }
    }
}

/// URL rewrite rule engine matching rules in priority order.
#[derive(Debug, Default, Clone)]
pub struct UrlRewriteEngine {
    rules: Vec<RewriteRule>,
}

impl UrlRewriteEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn with_rules(rules: Vec<RewriteRule>) -> Self {
        Self { rules }
    }

    pub fn add_rule(&mut self, rule: RewriteRule) {
        self.rules.push(rule);
    }

    pub fn add_direct(&mut self, pattern: &str, replacement: &str) -> Result<()> {
        self.rules.push(RewriteRule::direct(pattern, replacement)?);
        Ok(())
    }

    pub fn add_redirect_302(&mut self, pattern: &str, replacement: &str) -> Result<()> {
        self.rules.push(RewriteRule::redirect_302(pattern, replacement)?);
        Ok(())
    }

    pub fn add_redirect_307(&mut self, pattern: &str, replacement: &str) -> Result<()> {
        self.rules.push(RewriteRule::redirect_307(pattern, replacement)?);
        Ok(())
    }

    pub fn apply_rewrite(&self, url: &str) -> Option<RewriteOutcome> {
        for rule in &self.rules {
            if let Some(outcome) = rule.apply(url) {
                return Some(outcome);
            }
        }
        None
    }

    pub fn rules(&self) -> &[RewriteRule] { &self.rules }
    pub fn len(&self) -> usize { self.rules.len() }
    pub fn is_empty(&self) -> bool { self.rules.is_empty() }
    pub fn clear(&mut self) { self.rules.clear(); }
}

/// HTTP header modification action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeaderAction {
    Set { name: String, value: String },
    InjectIfNotPresent { name: String, value: String },
    Remove { name: String },
    ReplaceIfPresent { name: String, value: String },
}

/// Target HTTP phase for header modifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeaderTarget {
    Request,
    Response,
    Both,
}

/// Header rule with optional URL matching filter.
#[derive(Debug, Clone)]
pub struct HeaderRule {
    pub target: HeaderTarget,
    pub action: HeaderAction,
    pub url_pattern: Option<Regex>,
}

impl HeaderRule {
    pub fn new(target: HeaderTarget, action: HeaderAction) -> Self {
        Self { target, action, url_pattern: None }
    }

    pub fn with_url_filter(target: HeaderTarget, action: HeaderAction, pattern: &str) -> Result<Self> {
        let reg = Regex::new(pattern).with_context(|| format!("Invalid URL pattern: '{pattern}'"))?;
        Ok(Self { target, action, url_pattern: Some(reg) })
    }

    pub fn set(target: HeaderTarget, name: &str, value: &str) -> Self {
        Self::new(target, HeaderAction::Set { name: name.to_string(), value: value.to_string() })
    }

    pub fn inject(target: HeaderTarget, name: &str, value: &str) -> Self {
        Self::new(target, HeaderAction::InjectIfNotPresent { name: name.to_string(), value: value.to_string() })
    }

    pub fn remove(target: HeaderTarget, name: &str) -> Self {
        Self::new(target, HeaderAction::Remove { name: name.to_string() })
    }
}

/// Engine to inject, replace, or remove HTTP request/response headers.
#[derive(Debug, Default, Clone)]
pub struct HeaderModifier {
    rules: Vec<HeaderRule>,
}

impl HeaderModifier {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: HeaderRule) {
        self.rules.push(rule);
    }

    pub fn set_user_agent(&mut self, user_agent: &str) {
        self.rules.push(HeaderRule::set(HeaderTarget::Request, "User-Agent", user_agent));
    }

    pub fn set_referer(&mut self, referer: &str) {
        self.rules.push(HeaderRule::set(HeaderTarget::Request, "Referer", referer));
    }

    pub fn inject_cors_origin(&mut self, origin: &str) {
        self.rules.push(HeaderRule::set(HeaderTarget::Response, "Access-Control-Allow-Origin", origin));
    }

    pub fn set_custom_header(&mut self, target: HeaderTarget, name: &str, value: &str) {
        self.rules.push(HeaderRule::set(target, name, value));
    }

    pub fn remove_header(&mut self, target: HeaderTarget, name: &str) {
        self.rules.push(HeaderRule::remove(target, name));
    }

    pub fn modify_headers(&self, headers: &mut HashMap<String, String>, target: HeaderTarget, url: Option<&str>) {
        for rule in &self.rules {
            if rule.target != HeaderTarget::Both && rule.target != target {
                continue;
            }
            if let Some(ref pattern) = rule.url_pattern
                && !url.is_some_and(|u| pattern.is_match(u))
            {
                continue;
            }

            match &rule.action {
                HeaderAction::Set { name, value } => {
                    headers.retain(|k, _| !k.eq_ignore_ascii_case(name));
                    headers.insert(name.clone(), value.clone());
                }
                HeaderAction::InjectIfNotPresent { name, value } => {
                    if !headers.keys().any(|k| k.eq_ignore_ascii_case(name)) {
                        headers.insert(name.clone(), value.clone());
                    }
                }
                HeaderAction::Remove { name } => {
                    headers.retain(|k, _| !k.eq_ignore_ascii_case(name));
                }
                HeaderAction::ReplaceIfPresent { name, value } => {
                    if let Some(key) = headers.keys().find(|k| k.eq_ignore_ascii_case(name)).cloned() {
                        headers.remove(&key);
                        headers.insert(name.clone(), value.clone());
                    }
                }
            }
        }
    }

    pub fn modify_request_headers(&self, headers: &mut HashMap<String, String>, url: Option<&str>) {
        self.modify_headers(headers, HeaderTarget::Request, url);
    }

    pub fn modify_response_headers(&self, headers: &mut HashMap<String, String>, url: Option<&str>) {
        self.modify_headers(headers, HeaderTarget::Response, url);
    }

    pub fn rules(&self) -> &[HeaderRule] { &self.rules }
    pub fn len(&self) -> usize { self.rules.len() }
    pub fn is_empty(&self) -> bool { self.rules.is_empty() }
}

/// Predefined mock response payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MockResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl MockResponse {
    pub fn new(status_code: u16, headers: HashMap<String, String>, body: Vec<u8>) -> Self {
        Self { status_code, headers, body }
    }

    pub fn ok_json(value: &serde_json::Value) -> Result<Self> {
        let body = serde_json::to_vec(value).context("serialize JSON mock body")?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json; charset=utf-8".to_string());
        Ok(Self::new(200, headers, body))
    }

    pub fn ok_text(text: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/plain; charset=utf-8".to_string());
        Self::new(200, headers, text.as_bytes().to_vec())
    }

    pub fn not_found() -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/plain; charset=utf-8".to_string());
        Self::new(404, headers, b"404 Not Found".to_vec())
    }

    pub fn status_reason(&self) -> &'static str {
        match self.status_code {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            307 => "Temporary Redirect",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            500 => "Internal Server Error",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "Status",
        }
    }

    pub fn to_http_11_bytes(&self) -> Vec<u8> {
        let reason = self.status_reason();
        let mut out = format!("HTTP/1.1 {} {reason}\r\n", self.status_code);
        let mut has_content_len = false;
        let mut keys: Vec<&String> = self.headers.keys().collect();
        keys.sort();
        for key in keys {
            if key.eq_ignore_ascii_case("Content-Length") {
                has_content_len = true;
            }
            let val = &self.headers[key];
            out.push_str(&format!("{key}: {val}\r\n"));
        }
        if !has_content_len {
            out.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        }
        out.push_str("\r\n");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

/// Rule matching URL pattern to return a predefined mock response.
#[derive(Debug, Clone)]
pub struct MockResponseRule {
    pub url_pattern: Regex,
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl MockResponseRule {
    pub fn new(pattern: &str, status_code: u16, headers: HashMap<String, String>, body: Vec<u8>) -> Result<Self> {
        let reg = Regex::new(pattern).with_context(|| format!("Invalid regex: '{pattern}'"))?;
        Ok(Self { url_pattern: reg, status_code, headers, body })
    }

    pub fn json(pattern: &str, status_code: u16, json_val: &serde_json::Value) -> Result<Self> {
        let body = serde_json::to_vec(json_val).context("serialize JSON mock body")?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        Self::new(pattern, status_code, headers, body)
    }

    pub fn text(pattern: &str, status_code: u16, text_val: &str) -> Result<Self> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        Self::new(pattern, status_code, headers, text_val.as_bytes().to_vec())
    }

    pub fn matches(&self, url: &str) -> bool {
        self.url_pattern.is_match(url)
    }

    pub fn build_response(&self) -> MockResponse {
        MockResponse::new(self.status_code, self.headers.clone(), self.body.clone())
    }
}

/// Engine to evaluate mock response rules against incoming request URLs.
#[derive(Debug, Default, Clone)]
pub struct MockResponseEngine {
    rules: Vec<MockResponseRule>,
}

impl MockResponseEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn with_rules(rules: Vec<MockResponseRule>) -> Self {
        Self { rules }
    }

    pub fn add_rule(&mut self, rule: MockResponseRule) {
        self.rules.push(rule);
    }

    pub fn add_json_mock(&mut self, pattern: &str, status_code: u16, json: &serde_json::Value) -> Result<()> {
        self.rules.push(MockResponseRule::json(pattern, status_code, json)?);
        Ok(())
    }

    pub fn add_text_mock(&mut self, pattern: &str, status_code: u16, text: &str) -> Result<()> {
        self.rules.push(MockResponseRule::text(pattern, status_code, text)?);
        Ok(())
    }

    pub fn match_and_respond(&self, url: &str) -> Option<MockResponse> {
        for rule in &self.rules {
            if rule.matches(url) {
                return Some(rule.build_response());
            }
        }
        None
    }

    pub fn len(&self) -> usize { self.rules.len() }
    pub fn is_empty(&self) -> bool { self.rules.is_empty() }
    pub fn clear(&mut self) { self.rules.clear(); }
}

#[cfg(test)]
#[path = "pcap_exporter_test.rs"]
mod tests;
