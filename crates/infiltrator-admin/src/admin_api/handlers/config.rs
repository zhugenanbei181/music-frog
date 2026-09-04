//! Static config read-write endpoints for the generated mihomo config:
//! DNS, fake-ip, rule/proxy providers, sniffer, rules, and TUN
//! (`/admin/api/dns`, `/admin/api/fake-ip`, `/admin/api/*-providers`,
//! `/admin/api/sniffer`, `/admin/api/rules`, `/admin/api/tun`).

use axum::Json;
use infiltrator_core::{
    dns_io, fake_ip_io, proxy_providers, rules_io, sniffer, tun_io,
};
use infiltrator_domain::{dns, fake_ip, tun};
use infiltrator_domain::rules::{RuleProvidersPayload, RulesPayload};

use crate::admin_api::events::{
    AdminEvent, EVENT_DNS_CHANGED, EVENT_FAKE_IP_CHANGED, EVENT_PROXY_PROVIDERS_CHANGED,
    EVENT_RULE_PROVIDERS_CHANGED, EVENT_RULES_CHANGED, EVENT_SNIFFER_CHANGED, EVENT_TUN_CHANGED,
};
use crate::admin_api::models::*;
use crate::admin_api::state::{AdminApiContext, AdminApiState};

use super::schedule_rebuild;

pub async fn get_dns_config_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<dns::DnsConfig>, ApiError> {
    let config = dns_io::load_dns_config().await?;
    Ok(Json(config))
}

pub async fn save_dns_config_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<dns::DnsConfigPatch>,
) -> Result<Json<dns::DnsConfig>, ApiError> {
    let config = dns_io::save_dns_config(payload).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "dns-update");
    state.events.publish(AdminEvent::new(EVENT_DNS_CHANGED));
    Ok(Json(config))
}

pub async fn get_fake_ip_config_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<fake_ip::FakeIpConfig>, ApiError> {
    let config = fake_ip_io::load_fake_ip_config().await?;
    Ok(Json(config))
}

pub async fn save_fake_ip_config_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<fake_ip::FakeIpConfigPatch>,
) -> Result<Json<fake_ip::FakeIpConfig>, ApiError> {
    let config = fake_ip_io::save_fake_ip_config(payload).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "fake-ip-update");
    state.events.publish(AdminEvent::new(EVENT_FAKE_IP_CHANGED));
    Ok(Json(config))
}

pub async fn flush_fake_ip_cache_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<CacheFlushResponse>, ApiError> {
    let removed = fake_ip_io::clear_fake_ip_cache().await?;
    Ok(Json(CacheFlushResponse { removed }))
}

pub async fn get_rule_providers_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<RuleProvidersPayload>, ApiError> {
    let providers = rules_io::load_rule_providers().await?;
    Ok(Json(RuleProvidersPayload { providers }))
}

pub async fn save_rule_providers_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<RuleProvidersPayload>,
) -> Result<Json<RuleProvidersPayload>, ApiError> {
    let providers = rules_io::save_rule_providers(payload.providers).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "rule-providers-update");
    state
        .events
        .publish(AdminEvent::new(EVENT_RULE_PROVIDERS_CHANGED));
    Ok(Json(RuleProvidersPayload { providers }))
}

pub async fn get_proxy_providers_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<proxy_providers::ProxyProvidersPayload>, ApiError> {
    let providers = proxy_providers::load_proxy_providers().await?;
    Ok(Json(proxy_providers::ProxyProvidersPayload { providers }))
}

pub async fn save_proxy_providers_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<proxy_providers::ProxyProvidersPayload>,
) -> Result<Json<proxy_providers::ProxyProvidersPayload>, ApiError> {
    let providers = proxy_providers::save_proxy_providers(payload.providers).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "proxy-providers-update");
    state
        .events
        .publish(AdminEvent::new(EVENT_PROXY_PROVIDERS_CHANGED));
    Ok(Json(proxy_providers::ProxyProvidersPayload { providers }))
}

pub async fn get_sniffer_config_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let config = sniffer::load_sniffer_config().await?;
    Ok(Json(config))
}

pub async fn save_sniffer_config_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let config = sniffer::save_sniffer_config(payload).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "sniffer-update");
    state.events.publish(AdminEvent::new(EVENT_SNIFFER_CHANGED));
    Ok(Json(config))
}

pub async fn get_rules_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<RulesPayload>, ApiError> {
    let rules_list = rules_io::load_rules().await?;
    Ok(Json(RulesPayload { rules: rules_list }))
}

pub async fn save_rules_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<RulesPayload>,
) -> Result<Json<RulesPayload>, ApiError> {
    let rules_list = rules_io::save_rules(payload.rules).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "rules-update");
    state.events.publish(AdminEvent::new(EVENT_RULES_CHANGED));
    Ok(Json(RulesPayload { rules: rules_list }))
}

pub async fn get_tun_config_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<tun::TunConfig>, ApiError> {
    let config = tun_io::load_tun_config().await?;
    Ok(Json(config))
}

pub async fn save_tun_config_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<tun::TunConfigPatch>,
) -> Result<Json<tun::TunConfig>, ApiError> {
    let config = tun_io::save_tun_config(payload).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "tun-update");
    state.events.publish(AdminEvent::new(EVENT_TUN_CHANGED));
    Ok(Json(config))
}
