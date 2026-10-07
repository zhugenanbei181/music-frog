//! A single normalization of sandbox requests into domain facts and published data.
use super::TrafficContext;
use infiltrator_contract::error::Failure;
use infiltrator_contract::rule_trace_run::RuleTraceRequest;
use infiltrator_contract::rule_tracer::TrafficContextSnapshot;
use std::net::IpAddr;
impl TrafficContext {
    pub fn from_request(request: &RuleTraceRequest) -> Result<Self, Failure> {
        request.validate()?;
        let stored = &request.context;
        let mut context = Self::from_query(&request.query);
        context.domain = stored.domain.clone().or(context.domain);
        context.ip = stored
            .ip
            .as_deref()
            .map(|ip| {
                ip.trim()
                    .parse::<IpAddr>()
                    .expect("validated destination IP")
            })
            .or(context.ip);
        context.port = stored.port.or(context.port);
        context.src_ip = stored
            .src_ip
            .as_deref()
            .map(|ip| ip.trim().parse::<IpAddr>().expect("validated source IP"));
        context.src_port = stored.src_port;
        context.in_port = stored.in_port;
        context.in_type = stored.in_type.clone();
        context.in_name = stored.in_name.clone();
        context.in_user = stored.in_user.clone();
        context.process_name = stored.process_name.clone();
        context.process_path = stored.process_path.clone();
        context.network = stored.network.clone();
        context.dscp = stored.dscp;
        context.uid = stored.uid;
        context.package_name = stored.package_name.clone();
        context.client_ip = stored
            .client_ip
            .as_deref()
            .map(|ip| ip.trim().parse::<IpAddr>().expect("validated client IP"));
        Ok(context)
    }
    pub fn snapshot(&self) -> TrafficContextSnapshot {
        TrafficContextSnapshot {
            domain: self.domain.clone(),
            ip: self.ip.map(|ip| ip.to_string()),
            port: self.port,
            src_ip: self.src_ip.map(|ip| ip.to_string()),
            src_port: self.src_port,
            in_port: self.in_port,
            in_type: self.in_type.clone(),
            in_name: self.in_name.clone(),
            in_user: self.in_user.clone(),
            process_name: self.process_name.clone(),
            process_path: self.process_path.clone(),
            network: self.network.clone(),
            dscp: self.dscp,
            uid: self.uid,
            package_name: self.package_name.clone(),
            client_ip: self.client_ip.map(|ip| ip.to_string()),
        }
    }
}
