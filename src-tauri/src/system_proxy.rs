use infiltrator_desktop::{proxy as core_proxy, SystemProxyState};

pub(crate) fn apply_system_proxy(endpoint: Option<&str>) -> anyhow::Result<()> {
    core_proxy::apply_system_proxy(endpoint)
}

pub(crate) fn read_system_proxy_state() -> anyhow::Result<SystemProxyState> {
    core_proxy::read_system_proxy_state()
}
