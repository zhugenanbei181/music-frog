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
