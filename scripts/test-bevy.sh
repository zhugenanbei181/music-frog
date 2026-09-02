#!/usr/bin/env bash
set -euo pipefail

# Bevy UI 战略线（0.30 大一统）的独立验证入口。
#
# infiltrator-bevy-widgets 与 infiltrator-bevy-ui 是 standalone workspace
# （独立 lock/feature 闭包，0.20 构建面之外，见根 Cargo.toml 注释），根
# `scripts/test.sh` 的 --workspace 扫不到它们；本入口逐个跑行为测试与
# lint，并追加 bsn! 场景法机械守卫。尺寸对齐根入口：4 构建线程 / 4 测试线程。

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

crates=(
  "$repo_root/crates/infiltrator-bevy-widgets"
  "$repo_root/crates/infiltrator-bevy-ui"
)

for crate_dir in "${crates[@]}"; do
  echo "== bevy line: $crate_dir =="
  (
    cd "$crate_dir"
    cargo nextest run --build-jobs 4 --test-threads 4
    cargo clippy --all-targets -- -D warnings
    cargo fmt --check
  )
done

python3 "$repo_root/scripts/quality/bevy_bsn_guard.py" --mode enforce
