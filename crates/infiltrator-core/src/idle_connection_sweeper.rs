use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported transport protocols for tracked sessions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TransportProtocol {
    Tcp,
    Udp,
}

/// Represents an active network session to track.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TrackedSession {
    pub id: u64,
    pub protocol: TransportProtocol,
    pub src_addr: String,
    pub dst_addr: String,
    pub last_activity_secs: u64,
    pub bytes_transferred: u64,
}

/// Sweeps idle connections and limits concurrency.
pub struct IdleConnectionSweeper {
    tcp_idle_timeout_secs: u64,
    udp_idle_timeout_secs: u64,
    max_connections: usize,
    sessions: HashMap<u64, TrackedSession>,
}

impl IdleConnectionSweeper {
    /// Creates a new `IdleConnectionSweeper` with the given timeouts and capacity limits.
    pub fn new(
        tcp_idle_timeout_secs: u64,
        udp_idle_timeout_secs: u64,
        max_connections: usize,
    ) -> Self {
        Self {
            tcp_idle_timeout_secs,
            udp_idle_timeout_secs,
            max_connections,
            sessions: HashMap::new(),
        }
    }

    /// Registers a new session.
    /// Returns an error if the number of active connections is already at `max_connections`.
    pub fn register_session(&mut self, session: TrackedSession) -> Result<(), anyhow::Error> {
        if self.sessions.len() >= self.max_connections && !self.sessions.contains_key(&session.id) {
            return Err(anyhow::anyhow!("Maximum active connections reached"));
        }
        self.sessions.insert(session.id, session);
        Ok(())
    }

    /// Updates the last activity time and adds to the transferred bytes count of an existing session.
    pub fn touch_session(&mut self, id: u64, now_secs: u64, additional_bytes: u64) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.last_activity_secs = now_secs;
            session.bytes_transferred += additional_bytes;
        }
    }

    /// Sweeps idle connections that exceed their protocol's timeout.
    /// Returns a vector of IDs that were evicted.
    pub fn sweep_idle(&mut self, now_secs: u64) -> Vec<u64> {
        let mut evicted = Vec::new();
        let tcp_timeout = self.tcp_idle_timeout_secs;
        let udp_timeout = self.udp_idle_timeout_secs;

        self.sessions.retain(|id, session| {
            let timeout = match session.protocol {
                TransportProtocol::Tcp => tcp_timeout,
                TransportProtocol::Udp => udp_timeout,
            };

            let idle_duration = now_secs.saturating_sub(session.last_activity_secs);
            if idle_duration > timeout {
                evicted.push(*id);
                false
            } else {
                true
            }
        });

        evicted
    }

    /// Returns the current number of active connections.
    pub fn active_connection_count(&self) -> usize {
        self.sessions.len()
    }

    /// Explicitly removes a session by ID and returns it if it existed.
    pub fn remove_session(&mut self, id: u64) -> Option<TrackedSession> {
        self.sessions.remove(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_capacity() {
        let mut sweeper = IdleConnectionSweeper::new(60, 30, 2);

        let s1 = TrackedSession {
            id: 1,
            protocol: TransportProtocol::Tcp,
            src_addr: "192.168.1.1:10000".to_string(),
            dst_addr: "10.0.0.1:80".to_string(),
            last_activity_secs: 100,
            bytes_transferred: 0,
        };
        assert!(sweeper.register_session(s1.clone()).is_ok());
        assert_eq!(sweeper.active_connection_count(), 1);

        let mut s2 = s1.clone();
        s2.id = 2;
        s2.protocol = TransportProtocol::Udp;
        assert!(sweeper.register_session(s2.clone()).is_ok());
        assert_eq!(sweeper.active_connection_count(), 2);

        let mut s3 = s1.clone();
        s3.id = 3;
        assert!(sweeper.register_session(s3).is_err());
        assert_eq!(sweeper.active_connection_count(), 2);
    }

    #[test]
    fn test_touch_session() {
        let mut sweeper = IdleConnectionSweeper::new(60, 30, 10);
        let s = TrackedSession {
            id: 42,
            protocol: TransportProtocol::Tcp,
            src_addr: "192.168.1.1:10000".to_string(),
            dst_addr: "10.0.0.1:80".to_string(),
            last_activity_secs: 100,
            bytes_transferred: 50,
        };

        sweeper.register_session(s).unwrap();
        sweeper.touch_session(42, 150, 1024);

        let removed = sweeper.remove_session(42).unwrap();
        assert_eq!(removed.last_activity_secs, 150);
        assert_eq!(removed.bytes_transferred, 1074);
    }

    #[test]
    fn test_sweep_idle_differentiated_timeouts() {
        let mut sweeper = IdleConnectionSweeper::new(60, 30, 10);

        let s_tcp = TrackedSession {
            id: 1,
            protocol: TransportProtocol::Tcp,
            src_addr: "0.0.0.0:0".to_string(),
            dst_addr: "0.0.0.0:0".to_string(),
            last_activity_secs: 100,
            bytes_transferred: 0,
        };
        let s_udp = TrackedSession {
            id: 2,
            protocol: TransportProtocol::Udp,
            src_addr: "0.0.0.0:0".to_string(),
            dst_addr: "0.0.0.0:0".to_string(),
            last_activity_secs: 100,
            bytes_transferred: 0,
        };

        sweeper.register_session(s_tcp).unwrap();
        sweeper.register_session(s_udp).unwrap();

        // At now = 120, TCP idle (20) <= 60, UDP idle (20) <= 30
        let evicted = sweeper.sweep_idle(120);
        assert!(evicted.is_empty());
        assert_eq!(sweeper.active_connection_count(), 2);

        // At now = 140, TCP idle (40) <= 60, UDP idle (40) > 30 -> UDP evicted
        let evicted = sweeper.sweep_idle(140);
        assert_eq!(evicted.len(), 1);
        assert!(evicted.contains(&2));
        assert_eq!(sweeper.active_connection_count(), 1);

        // At now = 170, TCP idle (70) > 60 -> TCP evicted
        let evicted = sweeper.sweep_idle(170);
        assert_eq!(evicted.len(), 1);
        assert!(evicted.contains(&1));
        assert_eq!(sweeper.active_connection_count(), 0);
    }

    #[test]
    fn test_remove_session() {
        let mut sweeper = IdleConnectionSweeper::new(60, 30, 10);
        let s = TrackedSession {
            id: 99,
            protocol: TransportProtocol::Udp,
            src_addr: "1.2.3.4:5000".to_string(),
            dst_addr: "5.6.7.8:6000".to_string(),
            last_activity_secs: 10,
            bytes_transferred: 0,
        };
        sweeper.register_session(s.clone()).unwrap();
        assert_eq!(sweeper.active_connection_count(), 1);

        let removed = sweeper.remove_session(99).unwrap();
        assert_eq!(removed.id, 99);
        assert_eq!(sweeper.active_connection_count(), 0);

        // Remove again should return None
        assert!(sweeper.remove_session(99).is_none());
    }
}
