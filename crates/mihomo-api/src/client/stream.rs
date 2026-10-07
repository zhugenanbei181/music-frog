//! Lifecycle-aware controller WebSocket streams: one reconnecting
//! worker per endpoint, surfaced either as data-only receivers or as
//! [`StreamEvent`] lifecycle events.

use super::{MihomoClient, StreamEvent};
use crate::error::Result;
use crate::types::*;
use futures_util::StreamExt;
use infiltrator_contract::error::{ErrorCode, Failure};
use std::cmp::min;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

impl MihomoClient {
    async fn spawn_reconnecting_stream_events<T, F>(
        &self,
        endpoint: &str,
        query: Option<String>,
        parse: F,
    ) -> Result<UnboundedReceiver<StreamEvent<T>>>
    where
        T: Send + 'static,
        F: Fn(&str) -> Option<T> + Send + Sync + 'static,
    {
        let mut ws_url = self.base_url.clone();
        ws_url
            .set_scheme(if ws_url.scheme() == "https" {
                "wss"
            } else {
                "ws"
            })
            .ok();
        ws_url.set_path(endpoint);
        if let Some(q) = query {
            ws_url.set_query(Some(&q));
        }

        let (tx, rx) = unbounded_channel();
        let secret = self.secret.clone();
        let ws_url_str = ws_url.to_string();

        tokio::spawn(async move {
            let mut backoff = Duration::from_secs(1);
            loop {
                if tx.is_closed() {
                    break;
                }

                let _ = tx.send(StreamEvent::Connecting);

                let mut request = match ws_url_str.as_str().into_client_request() {
                    Ok(req) => req,
                    Err(error) => {
                        let _ = tx.send(StreamEvent::Failed(websocket_failure(error)));
                        break;
                    }
                };
                if let Some(s) = &secret {
                    let value = match format!("Bearer {}", s).parse() {
                        Ok(value) => value,
                        Err(_) => {
                            let _ = tx.send(StreamEvent::Failed(Failure::new(
                                ErrorCode::Configuration,
                                "controller secret cannot be encoded as an HTTP header",
                                false,
                            )));
                            break;
                        }
                    };
                    request.headers_mut().insert("Authorization", value);
                }

                let connected = tokio::select! {
                    connected = connect_async(request) => connected,
                    _ = tx.closed() => return,
                };
                let reconnect_reason = match connected {
                    Ok((ws_stream, _)) => {
                        backoff = Duration::from_secs(1);
                        let _ = tx.send(StreamEvent::Connected);
                        let (_, mut read) = ws_stream.split();
                        let mut reason =
                            Failure::new(ErrorCode::Network, "controller stream closed", true);
                        loop {
                            tokio::select! {
                                message = read.next() => match message {
                                    Some(Ok(Message::Text(text))) => {
                                        if let Some(item) = parse(text.as_ref())
                                            && tx.send(StreamEvent::Item(item)).is_err()
                                        {
                                            return;
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) | None => break,
                                    Some(Err(error)) => {
                                        reason = websocket_failure(error);
                                        break;
                                    }
                                    Some(Ok(_)) => {}
                                },
                                _ = tx.closed() => return,
                            }
                        }
                        reason
                    }
                    Err(error) => websocket_failure(error),
                };

                if !reconnect_reason.retryable {
                    let _ = tx.send(StreamEvent::Failed(reconnect_reason));
                    return;
                }
                if tx
                    .send(StreamEvent::Reconnecting(reconnect_reason))
                    .is_err()
                {
                    return;
                }
                if tx.is_closed() {
                    break;
                }
                tokio::select! {
                    _ = sleep(backoff) => {},
                    _ = tx.closed() => return,
                }
                backoff = min(backoff * 2, Duration::from_secs(30));
            }
        });

        Ok(rx)
    }

    async fn spawn_reconnecting_stream<T, F>(
        &self,
        endpoint: &str,
        query: Option<String>,
        parse: F,
    ) -> Result<UnboundedReceiver<T>>
    where
        T: Send + 'static,
        F: Fn(&str) -> Option<T> + Send + Sync + 'static,
    {
        let mut events = self
            .spawn_reconnecting_stream_events(endpoint, query, parse)
            .await?;
        let (tx, rx) = unbounded_channel();
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                if let StreamEvent::Item(item) = event
                    && tx.send(item).is_err()
                {
                    break;
                }
            }
        });
        Ok(rx)
    }

    pub async fn stream_logs_events(
        &self,
        level: Option<&str>,
    ) -> Result<UnboundedReceiver<StreamEvent<String>>> {
        self.spawn_reconnecting_stream_events(
            "/logs",
            level.map(|l| format!("level={}", l)),
            |text| Some(text.to_string()),
        )
        .await
    }

    pub async fn stream_traffic_events(
        &self,
    ) -> Result<UnboundedReceiver<StreamEvent<TrafficData>>> {
        self.spawn_reconnecting_stream_events("/traffic", None, |text| {
            serde_json::from_str::<TrafficData>(text).ok()
        })
        .await
    }

    pub async fn stream_connections_events(
        &self,
    ) -> Result<UnboundedReceiver<StreamEvent<ConnectionSnapshot>>> {
        self.spawn_reconnecting_stream_events("/connections", None, |text| {
            serde_json::from_str::<ConnectionSnapshot>(text).ok()
        })
        .await
    }

    pub async fn stream_logs(&self, level: Option<&str>) -> Result<UnboundedReceiver<String>> {
        self.spawn_reconnecting_stream("/logs", level.map(|l| format!("level={}", l)), |text| {
            Some(text.to_string())
        })
        .await
    }

    pub async fn stream_traffic(&self) -> Result<UnboundedReceiver<TrafficData>> {
        self.spawn_reconnecting_stream("/traffic", None, |text| {
            serde_json::from_str::<TrafficData>(text).ok()
        })
        .await
    }

    pub async fn stream_connections(&self) -> Result<UnboundedReceiver<ConnectionSnapshot>> {
        self.spawn_reconnecting_stream("/connections", None, |text| {
            serde_json::from_str::<ConnectionSnapshot>(text).ok()
        })
        .await
    }
}

fn websocket_failure(error: Error) -> Failure {
    let (code, retryable) = match &error {
        Error::Http(response) => match response.status().as_u16() {
            401 => (ErrorCode::Authentication, false),
            403 => (ErrorCode::Permission, false),
            400 | 422 => (ErrorCode::InvalidInput, false),
            404 | 405 => (ErrorCode::Unsupported, false),
            _ => (ErrorCode::Network, true),
        },
        Error::Url(_) | Error::HttpFormat(_) => (ErrorCode::Configuration, false),
        _ => (ErrorCode::Network, true),
    };
    let message = match &error {
        Error::Http(response) => format!("controller stream HTTP {}", response.status()),
        _ => error.to_string(),
    };
    Failure::new(code, message, retryable)
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
