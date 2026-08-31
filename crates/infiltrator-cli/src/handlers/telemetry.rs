use crate::context::Runtime;
use crate::output::{print_info, print_success};

/// Stream controller logs until Ctrl-C (or the controller closes the stream).
pub(crate) async fn logs(level: Option<&str>) -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    let client = runtime.api_client().await?;
    let mut receiver = client.stream_logs(level).await?;
    print_info("Streaming controller logs; press Ctrl-C to stop");
    loop {
        tokio::select! {
            line = receiver.recv() => match line {
                Some(line) => println!("{}", format_log_line(&line)),
                None => break,
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}

/// Stream live traffic rates until Ctrl-C.
pub(crate) async fn traffic() -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    let client = runtime.api_client().await?;
    let mut receiver = client.stream_traffic().await?;
    print_info("Streaming traffic; press Ctrl-C to stop");
    loop {
        tokio::select! {
            sample = receiver.recv() => match sample {
                Some(sample) => {
                    println!("↑ {}  ↓ {}", format_bytes(sample.up), format_bytes(sample.down));
                }
                None => break,
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}

/// Print the current core memory usage and exit.
pub(crate) async fn memory() -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    let client = runtime.api_client().await?;
    let memory = client.get_memory().await?;
    print_success(&format!(
        "memory in use: {} (system limit: {})",
        format_bytes(memory.in_use),
        format_bytes(memory.os_limit),
    ));
    Ok(())
}

/// Controller log frames are JSON objects (`{"type","payload"}`); anything
/// else is printed verbatim.
pub(crate) fn format_log_line(raw: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => format!(
            "[{}] {}",
            value["type"].as_str().unwrap_or("log"),
            value["payload"].as_str().unwrap_or(raw),
        ),
        Err(_) => raw.to_string(),
    }
}

const BYTE_UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];

/// Human-readable byte rate, e.g. `1024 -> 1.0 KiB`.
pub(crate) fn format_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < BYTE_UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", BYTE_UNITS[unit - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::{format_bytes, format_log_line};

    #[test]
    fn byte_formatting_covers_the_range() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    #[test]
    fn log_lines_are_formatted_from_controller_json() {
        let line = format_log_line(r#"{"type":"info","payload":"started"}"#);
        assert_eq!(line, "[info] started");
    }

    #[test]
    fn non_json_log_frames_pass_through_verbatim() {
        assert_eq!(format_log_line("plain text"), "plain text");
    }
}
