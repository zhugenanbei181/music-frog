#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ $# -gt 1 || ( $# -eq 1 && "$1" != "--no-run" ) ]]; then
  echo "usage: bash scripts/test.sh [--no-run]" >&2
  exit 2
fi

python3 scripts/quality/parity-guard.py --mode enforce
python3 scripts/quality/i18n-guard.py --mode enforce
python3 scripts/quality/session-guard.py --mode enforce
python3 scripts/quality/hot-reload-guard.py --mode enforce
python3 scripts/quality/crash-watchdog-guard.py --mode enforce
python3 scripts/quality/core-channel-guard.py --mode enforce
python3 scripts/quality/kernel-integrity-guard.py --mode enforce
python3 scripts/quality/kernel-rollback-guard.py --mode enforce
python3 scripts/quality/controller-auth-guard.py --mode enforce
python3 scripts/quality/core-log-level-guard.py --mode enforce
python3 scripts/quality/service-mode-guard.py --mode enforce
python3 scripts/quality/process-exit-guard.py --mode enforce
python3 scripts/quality/port-conflict-guard.py --mode enforce
python3 scripts/quality/core-resource-guard.py --mode enforce
python3 scripts/quality/offline-startup-guard.py --mode enforce
python3 scripts/quality/lifecycle-sync-guard.py --mode enforce
python3 scripts/quality/lifecycle-matrix-guard.py --mode enforce
python3 scripts/quality/tun-stack-guard.py --mode enforce
python3 scripts/quality/mtu-negotiation-guard.py --mode enforce
python3 scripts/quality/tun-routing-guard.py --mode enforce
python3 scripts/quality/system-proxy-guard.py --mode enforce
python3 scripts/quality/system-proxy-watchdog-guard.py --mode enforce
python3 scripts/quality/system-proxy-recovery-guard.py --mode enforce
python3 scripts/quality/lan-sharing-guard.py --mode enforce
python3 scripts/quality/lan-security-guard.py --mode enforce
python3 scripts/quality/ipv6-routing-guard.py --mode enforce
python3 scripts/quality/uwp-loopback-guard.py --mode enforce
python3 scripts/quality/pac-service-guard.py --mode enforce
python3 scripts/quality/network-roaming-guard.py --mode enforce
python3 scripts/quality/vpn-service-guard.py --mode enforce
python3 scripts/quality/system-toggle-guard.py --mode enforce
python3 scripts/quality/privileged-network-guard.py --mode enforce
python3 scripts/quality/traffic-waveform-guard.py --mode enforce
python3 scripts/quality/traffic-scale-guard.py --mode enforce
python3 scripts/quality/traffic-topology-guard.py --mode enforce
python3 scripts/quality/traffic-topology-navigation-guard.py --mode enforce
python3 scripts/quality/active-exit-guard.py --mode enforce
python3 scripts/quality/subscription-quota-guard.py --mode enforce
python3 scripts/quality/overview-master-switch-guard.py --mode enforce
python3 scripts/quality/proxy-mode-segment-guard.py --mode enforce
python3 scripts/quality/overview-speedtest-guard.py --mode enforce
python3 scripts/quality/overview-metrics-grid-guard.py --mode enforce
python3 scripts/quality/overview-ip-probe-guard.py --mode enforce
python3 scripts/quality/overview-card-reorder-guard.py --mode enforce
python3 scripts/quality/overview-reconnect-mask-guard.py --mode enforce
python3 scripts/quality/overview-responsive-viewport-guard.py --mode enforce
python3 scripts/quality/overview-regression-matrix-guard.py --mode enforce
python3 scripts/quality/proxies-five-group-guard.py --mode enforce
python3 scripts/quality/proxies-group-collapse-guard.py --mode enforce
python3 scripts/quality/proxies-node-selection-guard.py --mode enforce
python3 scripts/quality/proxies-filter-alive-guard.py --mode enforce
python3 scripts/quality/proxies-sorting-guard.py --mode enforce
python3 scripts/quality/proxies-favorite-pin-guard.py --mode enforce
python3 scripts/quality/proxies-protocol-chips-guard.py --mode enforce
python3 scripts/quality/proxies-latency-colors-guard.py --mode enforce
python3 scripts/quality/proxies-sparkline-guard.py --mode enforce
python3 scripts/quality/proxies-fuzzy-filter-guard.py --mode enforce
python3 scripts/quality/proxies-node-detail-drawer-guard.py --mode enforce
python3 scripts/quality/proxies-group-reorder-guard.py --mode enforce
python3 scripts/quality/proxies-compact-view-guard.py --mode enforce
python3 scripts/quality/proxies-skeleton-pulse-guard.py --mode enforce
python3 scripts/quality/proxies-regression-matrix-guard.py --mode enforce
python3 scripts/quality/rule-hit-audit-guard.py --mode enforce
python3 scripts/quality/rule-tracer-sandbox-guard.py --mode enforce
python3 scripts/quality/rule-tracer-override-guard.py --mode enforce
python3 scripts/quality/speedtest-parity-guard.py --mode enforce
python3 scripts/quality/speedtest-history-guard.py --mode enforce
python3 scripts/quality/speedtest-config-guard.py --mode enforce
python3 scripts/quality/speedtest-final-guard.py --mode enforce
python3 scripts/quality/subscription-lifecycle-guard.py --mode enforce
python3 scripts/quality/responsive-parity-guard.py --mode enforce
python3 scripts/quality/connections-audit-guard.py --mode enforce
python3 scripts/quality/rules-engine-guard.py --mode enforce
python3 scripts/quality/dns-studio-guard.py --mode enforce
python3 scripts/quality/aggregator-guard.py --mode enforce
python3 scripts/quality/yaml-diff-guard.py --mode enforce
python3 scripts/quality/multimodal-shell-guard.py --mode enforce

nextest_mode=()
if [[ $# -eq 1 ]]; then
  nextest_mode+=("--no-run")
fi

# Keep both dimensions explicit:
# - --build-jobs controls Cargo compilation parallelism.
# - --test-threads (nextest's -j alias) controls concurrent test processes.
exec cargo nextest run \
  --workspace \
  --build-jobs 4 \
  --test-threads 4 \
  "${nextest_mode[@]}"
