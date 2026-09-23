//! Lifecycle-aware controller WebSocket streams: one reconnecting
//! worker per endpoint, surfaced either as data-only receivers or as
//! [`StreamEvent`] lifecycle events.

use crate::error::Result;
use crate::types::*;
use futures_util::StreamExt;
use std::time::Duration;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

use super::{MihomoClient, StreamEvent};

impl MihomoClient {
    async fn spawn_reconnecting_stream_events<T, F>(
        &self,
        endpoint: &str,
        query: Option<String>,
        parse: F,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent<T>>>
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

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
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
                        let _ = tx.send(StreamEvent::Failed(error.to_string()));
                        break;
                    }
                };
                if let Some(s) = &secret {
                    let value = match format!("Bearer {}", s).parse() {
                        Ok(value) => value,
                        Err(_) => {
                            let _ = tx.send(StreamEvent::Failed(
                                "controller secret cannot be encoded as an HTTP header".to_owned(),
                            ));
                            break;
                        }
                    };
                    request.headers_mut().insert("Authorization", value);
                }

                let reconnect_reason = match connect_async(request).await {
                    Ok((ws_stream, _)) => {
                        backoff = Duration::from_secs(1);
                        let _ = tx.send(StreamEvent::Connected);
                        let (_, mut read) = ws_stream.split();
                        let mut reason = "controller stream closed".to_string();
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
                                        reason = error.to_string();
                                        break;
                                    }
                                    Some(Ok(_)) => {}
                                },
                                _ = tx.closed() => return,
                            }
                        }
                        reason
                    }
                    Err(error) => error.to_string(),
                };

                if tx
                    .send(StreamEvent::Reconnecting(reconnect_reason))
                    .is_err()
                {
                    return;
                }
                if tx.is_closed() {
                    break;
                }
                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
            }
        });

        Ok(rx)
    }

    async fn spawn_reconnecting_stream<T, F>(
        &self,
        endpoint: &str,
        query: Option<String>,
        parse: F,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<T>>
    where
        T: Send + 'static,
        F: Fn(&str) -> Option<T> + Send + Sync + 'static,
    {
        let mut events = self
            .spawn_reconnecting_stream_events(endpoint, query, parse)
            .await?;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
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
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent<String>>> {
        self.spawn_reconnecting_stream_events(
            "/logs",
            level.map(|l| format!("level={}", l)),
            |text| Some(text.to_string()),
        )
        .await
    }

    pub async fn stream_traffic_events(
        &self,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent<TrafficData>>> {
        self.spawn_reconnecting_stream_events("/traffic", None, |text| {
            serde_json::from_str::<TrafficData>(text).ok()
        })
        .await
    }

    pub async fn stream_connections_events(
        &self,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent<ConnectionSnapshot>>> {
        self.spawn_reconnecting_stream_events("/connections", None, |text| {
            serde_json::from_str::<ConnectionSnapshot>(text).ok()
        })
        .await
    }

    pub async fn stream_logs(
        &self,
        level: Option<&str>,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<String>> {
        self.spawn_reconnecting_stream("/logs", level.map(|l| format!("level={}", l)), |text| {
            Some(text.to_string())
        })
        .await
    }

    pub async fn stream_traffic(
        &self,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<TrafficData>> {
        self.spawn_reconnecting_stream("/traffic", None, |text| {
            serde_json::from_str::<TrafficData>(text).ok()
        })
        .await
    }

    pub async fn stream_connections(
        &self,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<ConnectionSnapshot>> {
        self.spawn_reconnecting_stream("/connections", None, |text| {
            serde_json::from_str::<ConnectionSnapshot>(text).ok()
        })
        .await
    }
}
