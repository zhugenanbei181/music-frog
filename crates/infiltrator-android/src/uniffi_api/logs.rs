//! Runtime log surface: an in-memory ring buffer of parsed mihomo log
//! lines plus streaming start/stop and buffered retrieval for Kotlin.

use std::sync::{Mutex, OnceLock};

use super::support::{build_controller_client, get_runtime, map_mihomo_error};
use crate::ffi::{FfiErrorCode, FfiStatus};

// --- Log Buffer ---

#[derive(Debug, Clone, uniffi::Enum)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Silent,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct LogsResult {
    pub status: FfiStatus,
    pub entries: Vec<LogEntry>,
}

struct LogBuffer {
    entries: Vec<LogEntry>,
    max_size: usize,
    is_streaming: bool,
}

impl LogBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_size),
            max_size,
            is_streaming: false,
        }
    }

    fn push(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    fn get_entries(&self, limit: usize) -> Vec<LogEntry> {
        let start = if self.entries.len() > limit {
            self.entries.len() - limit
        } else {
            0
        };
        self.entries[start..].to_vec()
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

fn log_buffer() -> &'static Mutex<LogBuffer> {
    static LOG_BUFFER: OnceLock<Mutex<LogBuffer>> = OnceLock::new();
    LOG_BUFFER.get_or_init(|| Mutex::new(LogBuffer::new(500)))
}

fn parse_log_level(level: &str) -> LogLevel {
    match level.to_lowercase().as_str() {
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warning" | "warn" => LogLevel::Warning,
        "error" => LogLevel::Error,
        "silent" => LogLevel::Silent,
        _ => LogLevel::Info,
    }
}

#[uniffi::export]
pub async fn logs_start_streaming() -> FfiStatus {
    get_runtime()
        .spawn(async move {
            // Check if already streaming
            {
                let buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                if buffer.is_streaming {
                    return FfiStatus::ok();
                }
            }

            // Set streaming flag
            {
                let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                buffer.is_streaming = true;
            }

            let client = match build_controller_client().await {
                Ok(c) => c,
                Err(e) => {
                    let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                    buffer.is_streaming = false;
                    return e;
                }
            };

            let rx = match client.stream_logs(Some("info")).await {
                Ok(rx) => rx,
                Err(e) => {
                    let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                    buffer.is_streaming = false;
                    return map_mihomo_error(e);
                }
            };

            tokio::spawn(async move {
                let mut rx = rx;
                while let Some(line) = rx.recv().await {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                        let level_str = parsed
                            .get("level")
                            .and_then(|v| v.as_str())
                            .unwrap_or("info");
                        let msg = parsed
                            .get("payload")
                            .or_else(|| parsed.get("message"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(&line)
                            .to_string();
                        let entry = LogEntry {
                            level: parse_log_level(level_str),
                            message: msg,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0),
                        };
                        let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                        buffer.push(entry);
                    }
                }
                let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                buffer.is_streaming = false;
            });

            FfiStatus::ok()
        })
        .await
        .unwrap_or_else(|e| FfiStatus::err(FfiErrorCode::Unknown, format!("runtime error: {}", e)))
}

#[uniffi::export]
pub fn logs_get(limit: u32) -> LogsResult {
    let buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
    LogsResult {
        status: FfiStatus::ok(),
        entries: buffer.get_entries(limit as usize),
    }
}

#[uniffi::export]
pub fn logs_clear() -> FfiStatus {
    let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
    buffer.clear();
    FfiStatus::ok()
}

#[uniffi::export]
pub fn logs_is_streaming() -> bool {
    let buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
    buffer.is_streaming
}
