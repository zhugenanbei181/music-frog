//! DUAL-14-08: the host echo-authority capability and the shared leak probe.
//!
//! A leak conclusion needs a *fact source*: a controlled echo authority that
//! answers a freshly generated subdomain with the resolver identity it
//! observed. [`DnsLeakEchoPort`] is that host capability — the concrete
//! adapter owns the network I/O (UDP / wire-format DoH / the platform
//! resolver). [`DnsLeakProbePort`] is the shared application boundary both
//! surfaces drive; it generates the random subdomains, queries every
//! configured source through the echo port, compares the observed identities
//! and publishes the typed report. Hosts without a fact source keep the empty
//! default and both surfaces publish a typed unsupported state instead of a
//! fabricated leak verdict (the old panel's hardcoded `country`/`isp`).

use async_trait::async_trait;
use infiltrator_contract::dns_leak::{DnsLeakEchoReport, DnsLeakEchoRequest, DnsLeakReport};

use crate::error::PortError;

#[async_trait]
pub trait DnsLeakEchoPort: Send + Sync {
    /// Resolve every requested subdomain through its configured resolver and
    /// report the identity the echo authority observed. A per-source failure
    /// is a result, not an error: the port only fails when it cannot run the
    /// probe at all.
    async fn observe(&self, request: DnsLeakEchoRequest) -> Result<DnsLeakEchoReport, PortError>;
}

#[async_trait]
pub trait DnsLeakProbePort: Send + Sync {
    /// Generate a fresh subdomain per configured source, observe the resolver
    /// identity each authority reports and publish the typed cross-source
    /// conclusion. Fails only when this host cannot cross-check at all.
    async fn probe(&self) -> Result<DnsLeakReport, PortError>;
}
