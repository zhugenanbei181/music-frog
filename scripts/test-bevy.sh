#!/usr/bin/env bash
set -euo pipefail

# Bevy UI 战略线的验证入口。
#
# infiltrator-bevy-widgets 与 infiltrator-bevy-ui 已纳入主 workspace，
# 本脚本对两个 crate 运行行为测试与 lint，并追加 bsn! 场景法机械守卫。

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

crates=(
  "infiltrator-bevy-widgets"
  "infiltrator-bevy-ui"
)

for crate_name in "${crates[@]}"; do
  echo "== bevy line: $crate_name =="
  cargo nextest run -p "$crate_name" --build-jobs 4 --test-threads 4
  cargo clippy -p "$crate_name" --all-targets -- -D warnings
  cargo fmt -p "$crate_name" --check
done

python3 "$repo_root/scripts/quality/bevy_bsn_guard.py" --mode enforce
python3 "$repo_root/scripts/quality/parity-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/session-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/hot-reload-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/crash-watchdog-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/core-channel-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/kernel-integrity-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/kernel-rollback-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/controller-auth-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/core-log-level-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/service-mode-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/process-exit-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/port-conflict-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/core-resource-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/offline-startup-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/lifecycle-sync-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/lifecycle-matrix-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/tun-stack-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/mtu-negotiation-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/tun-routing-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/system-proxy-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/system-proxy-watchdog-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/system-proxy-recovery-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/lan-sharing-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/lan-security-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/ipv6-routing-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/uwp-loopback-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/pac-service-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/network-roaming-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/vpn-service-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/system-toggle-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/privileged-network-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/traffic-waveform-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/traffic-scale-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/traffic-topology-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/traffic-topology-navigation-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/active-exit-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/subscription-quota-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/overview-master-switch-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxy-mode-segment-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/overview-speedtest-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/overview-metrics-grid-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/overview-ip-probe-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/overview-card-reorder-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/overview-reconnect-mask-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/overview-responsive-viewport-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/overview-regression-matrix-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-five-group-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-group-collapse-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-node-selection-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-filter-alive-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-sorting-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-favorite-pin-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-protocol-chips-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-latency-colors-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-sparkline-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-fuzzy-filter-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-node-detail-drawer-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-group-reorder-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-compact-view-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-skeleton-pulse-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/proxies-regression-matrix-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/rule-hit-audit-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/rule-tracer-sandbox-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/rule-tracer-override-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/speedtest-parity-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/speedtest-history-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/speedtest-config-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/speedtest-final-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/subscription-lifecycle-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/responsive-parity-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/connections-audit-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/rules-engine-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/dns-studio-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/aggregator-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/yaml-diff-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/multimodal-shell-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/protocol-ecosystem-guard.py" --mode enforce
python3 "$repo_root/scripts/quality/notification-timeout-guard.py" --mode enforce
