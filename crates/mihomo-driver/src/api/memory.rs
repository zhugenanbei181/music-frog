//! One-shot reader for Mihomo's continuously streaming `/memory` endpoint.

use super::client::MihomoClient;
use crate::error::{MihomoError, Result};
use crate::types::MemoryData;
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use std::io::{self, ErrorKind};
use std::result;

const MAX_MEMORY_SAMPLE_BYTES: usize = 64 * 1024;

impl MihomoClient {
    /// Read the first complete, nonzero memory sample and then drop the response body.
    /// Mihomo keeps `/memory` open and pushes a JSON sample every second, so
    /// `Response::json()` would wait for an end-of-stream that never arrives.
    pub async fn get_memory(&self) -> Result<MemoryData> {
        let url = self.build_url("/memory")?;
        let response = self
            .add_auth(self.client.get(url))
            .send()
            .await?
            .error_for_status()?;
        let mut chunks = response.bytes_stream();
        let mut sample = Vec::new();
        while let Some(chunk) = chunks.next().await {
            sample.extend_from_slice(&chunk?);
            while let Some((memory, consumed)) = next_json_value::<MemoryData>(&sample)? {
                sample.drain(..consumed);
                // The controller's sampler can publish an initial zero before
                // its first process-memory read. A running Mihomo process
                // cannot have a real zero-byte working set, so keep reading
                // until the first actual sample arrives.
                if memory.in_use > 0 {
                    return Ok(memory);
                }
            }
            if sample.len() > MAX_MEMORY_SAMPLE_BYTES {
                return Err(MihomoError::Io(io::Error::new(
                    ErrorKind::InvalidData,
                    "Mihomo memory sample exceeded the 64 KiB limit",
                )));
            }
        }
        Err(MihomoError::Io(io::Error::new(
            ErrorKind::UnexpectedEof,
            "Mihomo memory stream ended before its first complete sample",
        )))
    }
}

fn next_json_value<T: DeserializeOwned>(
    bytes: &[u8],
) -> result::Result<Option<(T, usize)>, serde_json::Error> {
    let mut values = serde_json::Deserializer::from_slice(bytes).into_iter::<T>();
    match values.next() {
        Some(Ok(value)) => Ok(Some((value, values.byte_offset()))),
        Some(Err(error)) if error.is_eof() => Ok(None),
        Some(Err(error)) => Err(error),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::next_json_value;
    use crate::types::MemoryData;

    #[test]
    fn memory_sample_parser_waits_for_a_complete_json_value() {
        assert!(
            next_json_value::<MemoryData>(b"{\"inuse\":12")
                .unwrap()
                .is_none()
        );
        let (memory, consumed) = next_json_value::<MemoryData>(b"{\"inuse\":12,\"oslimit\":0}\n")
            .unwrap()
            .expect("complete first sample");
        assert_eq!(memory.in_use, 12);
        assert_eq!(memory.os_limit, 0);
        assert!(consumed > 0);
    }

    #[test]
    fn memory_sample_parser_returns_the_first_sample_from_a_continuous_body() {
        let (memory, consumed) = next_json_value::<MemoryData>(
            b"{\"inuse\":12,\"oslimit\":0}\n{\"inuse\":99,\"oslimit\":0}\n",
        )
        .unwrap()
        .expect("first complete sample");
        assert_eq!(memory.in_use, 12);
        assert!(consumed < b"{\"inuse\":12,\"oslimit\":0}\n{\"inuse\":99,\"oslimit\":0}\n".len());
    }

    #[test]
    fn malformed_complete_memory_samples_are_not_silently_ignored() {
        assert!(next_json_value::<MemoryData>(b"{\"inuse\":\"bad\",\"oslimit\":0}").is_err());
    }
}
