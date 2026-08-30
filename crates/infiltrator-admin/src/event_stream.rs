use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Event types for the Admin SSE stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum StreamEventType {
    TrafficUpdate { up_bps: u64, down_bps: u64 },
    LogEntry { level: String, message: String },
    ConnectionCount(usize),
    Heartbeat,
}

/// Broadcasts admin events to connected SSE clients.
#[derive(Clone)]
pub struct AdminEventBroadcaster {
    sender: broadcast::Sender<StreamEventType>,
}

impl AdminEventBroadcaster {
    /// Creates a new broadcaster with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Broadcasts an event to all subscribed clients.
    /// Returns the number of receivers that received the message.
    pub fn broadcast(&self, event: StreamEventType) -> usize {
        match self.sender.send(event) {
            Ok(count) => count,
            Err(_) => 0,
        }
    }

    /// Subscribes to the broadcast channel.
    pub fn subscribe(&self) -> broadcast::Receiver<StreamEventType> {
        self.sender.subscribe()
    }

    /// Formats an event as a Server-Sent Events (SSE) data frame.
    pub fn format_sse(event: &StreamEventType) -> String {
        let json = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
        format!("data: {}\n\n", json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast::error::RecvError;

    #[tokio::test]
    async fn test_broadcast_and_receive() {
        let broadcaster = AdminEventBroadcaster::new(10);
        let mut rx1 = broadcaster.subscribe();
        let mut rx2 = broadcaster.subscribe();

        let event = StreamEventType::Heartbeat;
        let receivers = broadcaster.broadcast(event.clone());
        assert_eq!(receivers, 2);

        assert_eq!(rx1.recv().await.unwrap(), event);
        assert_eq!(rx2.recv().await.unwrap(), event);
    }

    #[test]
    fn test_format_sse_traffic_update() {
        let event = StreamEventType::TrafficUpdate { up_bps: 100, down_bps: 200 };
        let sse = AdminEventBroadcaster::format_sse(&event);
        assert!(sse.starts_with("data: {"));
        assert!(sse.ends_with("}\n\n"));
        assert!(sse.contains(r#""type":"TrafficUpdate""#));
        assert!(sse.contains(r#""up_bps":100"#));
        assert!(sse.contains(r#""down_bps":200"#));
    }

    #[test]
    fn test_format_sse_connection_count() {
        let event = StreamEventType::ConnectionCount(42);
        let sse = AdminEventBroadcaster::format_sse(&event);
        assert!(sse.contains(r#""type":"ConnectionCount""#));
        assert!(sse.contains(r#""data":42"#));
    }

    #[test]
    fn test_format_sse_log_entry() {
        let event = StreamEventType::LogEntry {
            level: "INFO".to_string(),
            message: "Started".to_string(),
        };
        let sse = AdminEventBroadcaster::format_sse(&event);
        assert!(sse.contains(r#""type":"LogEntry""#));
        assert!(sse.contains(r#""level":"INFO""#));
        assert!(sse.contains(r#""message":"Started""#));
    }

    #[tokio::test]
    async fn test_capacity_overflow() {
        let broadcaster = AdminEventBroadcaster::new(2);
        let mut rx = broadcaster.subscribe();

        // Broadcast 3 events while capacity is 2
        broadcaster.broadcast(StreamEventType::ConnectionCount(1));
        broadcaster.broadcast(StreamEventType::ConnectionCount(2));
        broadcaster.broadcast(StreamEventType::ConnectionCount(3));

        // The first event should be dropped due to lag
        match rx.recv().await {
            Err(RecvError::Lagged(n)) => assert_eq!(n, 1),
            _ => panic!("Expected Lagged error"),
        }

        // We should then get the remaining two events in order
        assert_eq!(rx.recv().await.unwrap(), StreamEventType::ConnectionCount(2));
        assert_eq!(rx.recv().await.unwrap(), StreamEventType::ConnectionCount(3));
    }
}
