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
