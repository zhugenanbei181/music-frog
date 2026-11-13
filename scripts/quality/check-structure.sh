#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# The structure gate is the only executor of every mandatory standard in this
# repository, so its own reachability is a product fact (GATERUN-001).
#
# It used to run under `set -e` as a flat list of 193 legs: the first leg that
# returned non-zero aborted the whole script, and every leg after it silently
# never executed - locally or in `main-gate.yml`. That is how a user-visible
# defect (`I18NNAV-001`, red `i18n-guard --mode enforce` at line ~113) survived
# 121 batches while the gate itself was reported as "one known red leg".
#
# The runner below is therefore **collecting**: every leg runs, its exit status
# and first failure signature are recorded, and the batch is summarised once at
# the end. The gate still exits non-zero when any leg fails - a red standard is
# never downgraded to green - but `scripts/quality/structure_gate_ledger.json`
# must declare each currently-red leg with an owner and a written reason, so a
# new red leg cannot hide behind an old one and a repaired declaration cannot
# go stale. `structure-gate-guard.py` proves this file keeps that shape.
# ---------------------------------------------------------------------------
set -uo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

GATE_LEDGER="scripts/quality/structure_gate_ledger.json"
GATE_LOG_DIR="$(mktemp -d "${TMPDIR:-/tmp}/structure-gate.XXXXXX")"
trap 'rm -rf "$GATE_LOG_DIR"' EXIT

GATE_INDEX=0
FAILED_LEGS=0

# gate <command...> - run one leg, stream its output, and record the outcome.
GATE_FAILURES="$GATE_LOG_DIR/failures.tsv"
: > "$GATE_FAILURES"

gate() {
  local name log rc signature
  name="$(printf '%q ' "$@")"
  name="${name% }"
  GATE_INDEX=$((GATE_INDEX + 1))
  log="$GATE_LOG_DIR/$GATE_INDEX.log"
  # The leg's own output stays on stdout, exactly as before; the marker line is
  # appended so the exit status survives the pipeline.
  { "$@" 2>&1; echo "__gate_rc__$?"; } | tee "$log" | { grep -v '^__gate_rc__[0-9]*$' || true; }
  rc="$(sed -n 's/^__gate_rc__\([0-9]*\)$/\1/p' "$log" | tail -n 1)"
  if [ "${rc:-missing}" != "0" ]; then
    FAILED_LEGS=$((FAILED_LEGS + 1))
    signature="$(grep -m 1 -Ei 'violation|failed|error|missing|stale|not pruned|panic' "$log" || true)"
    printf 'c%s\t%s\t%s\n' "$GATE_INDEX" "$name" "${signature:0:200}" >> "$GATE_FAILURES"
  fi
}

gate bash scripts/check-test-policy.sh
gate python3 scripts/quality/import-guard.py --self-test
gate python3 scripts/quality/import-guard.py --mode enforce
gate python3 scripts/quality/slint-source-import-guard.py
gate python3 scripts/quality/slint-syntax-guard.py --self-test
gate python3 scripts/quality/slint-syntax-guard.py --mode enforce
gate python3 scripts/quality/slint-typography-role-guard.py --self-test
gate python3 scripts/quality/slint-typography-role-guard.py --mode enforce
gate python3 scripts/quality/slint-dead-input-guard.py --self-test
gate python3 scripts/quality/slint-dead-input-guard.py --mode enforce
gate python3 scripts/quality/core-boundary-guard.py --mode enforce
gate python3 scripts/quality/adapter-operation-declarations-guard.py
gate python3 scripts/quality/workspace-pin-guard.py --mode enforce
gate python3 scripts/quality/bevy_bsn_guard.py --mode enforce
gate python3 scripts/quality/bevy_rem_guard.py --mode enforce
# Bevy 场景律法（BSN 唯一构造 + rem 唯一 authored 长度）的执行器随共享包
# zhuge-ui-kit 一起发布；此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
gate python3 scripts/quality/bevy_scene_gate.py \
  --root crates/infiltrator-bevy-ui/src \
  --root crates/infiltrator-bevy-widgets/src \
  --baseline scripts/quality/bevy_scene_ratchet.json
gate python3 -m unittest discover -s scripts/quality/tests -p 'test_bevy_guard_owner_binding.py'
# 带宽算术唯一律（最宽带只有一个 owner）的执行器随共享包 zhuge-ui-core 一起发布；
# 此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
gate python3 scripts/quality/band_arithmetic_gate.py \
  --root crates \
  --baseline scripts/quality/band_arithmetic_ratchet.json
gate python3 scripts/quality/test_band_arithmetic_gate.py
# 区间归一化唯一律（进入单位区间的投影只有一个 owner）的执行器随共享包 zhuge-ui-core
# 一起发布；此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
gate python3 scripts/quality/interval_normalization_gate.py \
  --root crates \
  --baseline scripts/quality/interval_normalization_ratchet.json
gate python3 scripts/quality/test_interval_normalization_gate.py
# 标量混合唯一律（`A + (B - A) * t` 只有一个 owner）的执行器随共享包 zhuge-ui-core
# 一起发布；此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
gate python3 scripts/quality/scalar_mix_gate.py \
  --root crates \
  --baseline scripts/quality/scalar_mix_ratchet.json
gate python3 scripts/quality/test_scalar_mix_gate.py
# 缓动斜坡与 Bézier 求解唯一律（`smoothstep` 形状与「按进度解曲线参数」各只有一个 owner）的执行器
# 随共享包 zhuge-ui-core 一起发布；此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
gate python3 scripts/quality/easing_owner_gate.py \
  --root crates \
  --baseline scripts/quality/easing_owner_ratchet.json
gate python3 scripts/quality/test_easing_owner_gate.py
gate python3 scripts/quality/bevy_shader_guard.py --mode enforce
gate python3 scripts/quality/line-guard.py --self-test
gate python3 scripts/quality/line-guard.py --mode enforce
gate python3 scripts/quality/kt-line-guard.py --self-test
gate python3 scripts/quality/kt-line-guard.py --mode enforce
gate python3 scripts/quality/doc-governance-guard.py --self-test
gate python3 scripts/quality/doc-governance-guard.py --mode enforce
gate python3 scripts/quality/test-layout-guard.py --mode enforce
gate python3 scripts/quality/mod-rs-guard.py --self-test
gate python3 scripts/quality/mod-rs-guard.py --mode enforce
gate python3 scripts/quality/typed-state-guard.py --self-test
gate python3 scripts/quality/typed-state-guard.py --mode enforce
gate python3 scripts/quality/screenshot-identity-guard.py --self-test
gate python3 scripts/quality/screenshot-identity-guard.py --mode enforce
gate python3 scripts/quality/receipt-freshness-guard.py --self-test
gate python3 scripts/quality/receipt-freshness-guard.py --mode enforce
gate python3 scripts/quality/capture-corpus-integrity-guard.py --self-test
gate python3 scripts/quality/capture-corpus-integrity-guard.py --mode enforce
gate python3 scripts/quality/todo-status-freshness-guard.py --self-test
gate python3 scripts/quality/todo-status-freshness-guard.py --mode enforce
gate python3 scripts/quality/retired-surface-guard.py
gate python3 -m unittest discover -s scripts/quality/tests -p 'test_easing_owner_tests_mutation.py'
gate python3 scripts/quality/lurker-neutrality-guard.py --self-test
gate python3 scripts/quality/lurker-neutrality-guard.py
gate python3 scripts/quality/publish-order-guard.py --self-test
gate python3 scripts/quality/publish-order-guard.py
gate python3 scripts/quality/rustdoc-guard.py --self-test
gate python3 scripts/quality/rustdoc-guard.py --mode structure
gate python3 scripts/quality/rustdoc-guard.py --mode docs
gate python3 scripts/quality/neutral-release-guard.py --self-test
gate python3 scripts/quality/neutral-release-guard.py --mode structure
gate python3 scripts/quality/closure-questions-guard.py --self-test
gate python3 scripts/quality/closure-questions-guard.py
gate python3 scripts/quality/android-bevy-driver-dependency-guard.py --self-test
gate python3 scripts/quality/android-bevy-driver-dependency-guard.py
gate python3 scripts/quality/mobile-cfg-discipline-guard.py --self-test
gate python3 scripts/quality/mobile-cfg-discipline-guard.py --mode enforce
gate python3 scripts/quality/bevy-desktop-host-boundary-guard.py --self-test
gate python3 scripts/quality/bevy-desktop-host-boundary-guard.py
# 侧栏导航律法：Bevy 的侧栏列只挂导航列，行来自契约注册表，选中态由 widget 层单一作者绘制。
gate python3 scripts/quality/bevy-sidebar-nav-guard.py --self-test
gate python3 scripts/quality/bevy-sidebar-nav-guard.py --mode enforce
# 代理节点网格律法：Bevy 只读共享页身宽规则，不按窗口档私画 1/2/3/4 列。
gate python3 scripts/quality/bevy-proxy-grid-owner-guard.py --self-test
gate python3 scripts/quality/bevy-proxy-grid-owner-guard.py --mode enforce
# 公共 IP 探针律法：dispatch / owner / SurfaceReader 三者必须形成真实状态闭环。
gate python3 scripts/quality/public-ip-probe-owner-guard.py --self-test
gate python3 scripts/quality/public-ip-probe-owner-guard.py --mode enforce
# 同一律法在 Iced 侧栏：列只挂导航，行集唯一来自 shell_navigation 注册表，无路由→字形表。
gate python3 scripts/quality/iced-sidebar-nav-guard.py --self-test
gate python3 scripts/quality/iced-sidebar-nav-guard.py --mode enforce
gate python3 scripts/quality/iced-observation-routes-guard.py --self-test
gate python3 scripts/quality/iced-observation-routes-guard.py --mode enforce
gate python3 scripts/quality/iced-dead-input-guard.py --self-test
gate python3 scripts/quality/iced-dead-input-guard.py --mode enforce
gate python3 scripts/quality/android-owner-client-guard.py --self-test
gate python3 scripts/quality/android-owner-client-guard.py
gate python3 scripts/quality/android-vpn-lifecycle-guard.py --self-test
gate python3 scripts/quality/android-vpn-lifecycle-guard.py
gate python3 scripts/quality/android-manifest-contract-guard.py
gate python3 scripts/quality/android-architecture-redline-guard.py --self-test
gate python3 scripts/quality/android-architecture-redline-guard.py --mode enforce
gate python3 scripts/quality/android-so-combination-guard.py --self-test
gate python3 scripts/quality/android-so-combination-guard.py
gate python3 scripts/quality/real-process-evidence-guard.py --self-test
gate python3 scripts/quality/real-process-evidence-guard.py
gate python3 scripts/quality/android-interaction-receipt-guard.py --self-test
gate python3 scripts/quality/android-interaction-receipt-guard.py --mode enforce
gate python3 scripts/quality/verify-packaging.py
gate python3 -m unittest discover -s scripts/packaging_tests -p 'test_*.py'
gate python3 -m unittest discover -s scripts/quality/tests -p 'test_*.py'
gate python3 -m unittest discover -s scripts/quality/tests -p 'test_slint_mobile_chrome_guard.py'
gate python3 scripts/quality/i18n-guard.py --mode enforce
gate python3 scripts/quality/design-resource-authority.py --self-test
gate python3 scripts/quality/design-resource-authority.py --skip-exporter
gate python3 scripts/quality/shared-copy-guard.py --mode enforce
gate python3 scripts/quality/shared-copy-guard.py --self-test
gate python3 scripts/quality/scrollbar-contract-guard.py --mode enforce
gate python3 scripts/quality/scrollbar-contract-guard.py --self-test
gate python3 scripts/quality/slint-mobile-chrome-guard.py --mode enforce
gate python3 scripts/quality/slint-mobile-chrome-guard.py --self-test
gate python3 scripts/quality/core-session-guard.py --mode enforce
gate python3 scripts/quality/core-session-guard.py --self-test
gate python3 scripts/quality/slint-fact-row-guard.py --mode enforce
gate python3 scripts/quality/slint-fact-row-guard.py --self-test
gate python3 scripts/quality/slint-padding-guard.py --mode enforce
gate python3 scripts/quality/slint-padding-guard.py --self-test
gate python3 scripts/quality/slint-dock-material-guard.py --mode enforce
gate python3 scripts/quality/slint-dock-material-guard.py --self-test
gate python3 scripts/quality/slint-surface-ladder-guard.py --mode enforce
gate python3 scripts/quality/slint-surface-ladder-guard.py --self-test
gate python3 scripts/quality/slint-viewport-tier-guard.py --mode enforce
gate python3 scripts/quality/slint-viewport-tier-guard.py --self-test
gate python3 scripts/quality/slint-list-page-guard.py --mode enforce
gate python3 scripts/quality/slint-list-page-guard.py --self-test
gate python3 scripts/parity/matrix_size.py
gate python3 scripts/parity/space_utilization.py --self-test
gate python3 scripts/parity/space_utilization.py --check-export
gate python3 scripts/parity/space_utilization.py --enforce
gate python3 scripts/parity/space_utilization.py --surface iced --enforce
# The unified matrix is a committed artifact: this leg proves the generated
# rows are still there, in generated order, with the columns they own. The
# Bevy matrix is left out until its own session lands the current rewrite.
gate python3 scripts/parity/generate_unified_matrix.py --check --surface=slint --surface=iced
gate python3 scripts/quality/slint-workbench-row-guard.py --mode enforce
gate python3 scripts/quality/slint-workbench-row-guard.py --self-test
gate python3 scripts/quality/slint-selector-segment-guard.py --mode enforce
gate python3 scripts/quality/slint-selector-segment-guard.py --self-test
gate python3 scripts/quality/slint-dashboard-grid-guard.py --mode enforce
gate python3 scripts/quality/slint-dashboard-grid-guard.py --self-test
gate python3 scripts/quality/slint-action-label-guard.py --mode enforce
gate python3 scripts/quality/slint-action-label-guard.py --self-test
gate python3 scripts/quality/slint-rail-edge-dialect-guard.py --mode enforce
gate python3 scripts/quality/slint-rail-edge-dialect-guard.py --self-test
gate python3 scripts/quality/slint-sidebar-separator-guard.py --mode enforce
gate python3 scripts/quality/slint-sidebar-separator-guard.py --self-test
gate python3 scripts/quality/slint-navigation-owner-guard.py --mode enforce
gate python3 scripts/quality/slint-navigation-owner-guard.py --self-test
gate python3 scripts/quality/slint-flag-shape-guard.py --mode enforce
gate python3 scripts/quality/slint-flag-shape-guard.py --self-test
gate python3 scripts/quality/flag-plate-guard.py --mode enforce
gate python3 scripts/quality/flag-plate-guard.py --self-test
gate python3 scripts/quality/structure-gate-guard.py --self-test
gate python3 scripts/quality/slint-theme-channel-guard.py --mode enforce
gate python3 scripts/quality/slint-theme-channel-guard.py --self-test
gate python3 scripts/quality/android-product-identity-guard.py
gate python3 scripts/quality/release-build-target-guard.py --self-test
gate python3 scripts/quality/release-build-target-guard.py
gate python3 scripts/quality/android-mihomo-assets-guard.py
gate python3 scripts/quality/apple-ci-deferral-guard.py
gate python3 scripts/quality/ci-trigger-guard.py
gate python3 scripts/quality/license-guard.py --self-test
gate python3 scripts/quality/license-guard.py --mode enforce
gate python3 scripts/quality/parity-coverage-guard.py --self-test
gate python3 scripts/quality/parity-coverage-guard.py --mode enforce
gate python3 scripts/quality/flclash-core-parity-guard.py --self-test
gate python3 scripts/quality/flclash-core-parity-guard.py --mode enforce
gate python3 scripts/quality/flclash-dashboard-widget-guard.py --self-test
gate python3 scripts/quality/flclash-dashboard-widget-guard.py --mode enforce
gate python3 scripts/quality/flclash-sidebar-expansion-guard.py --self-test
gate python3 scripts/quality/flclash-sidebar-expansion-guard.py --mode enforce
gate python3 scripts/quality/flclash-content-ground-guard.py --self-test
gate python3 scripts/quality/flclash-content-ground-guard.py --mode enforce
gate python3 scripts/quality/flclash-sidebar-void-guard.py --self-test
gate python3 scripts/quality/flclash-sidebar-void-guard.py --mode enforce
gate python3 scripts/quality/flclash-shell-reserve-guard.py --self-test
gate python3 scripts/quality/flclash-shell-reserve-guard.py --mode enforce
gate python3 scripts/quality/sidebar-expansion-owner-guard.py --self-test
gate python3 scripts/quality/sidebar-expansion-owner-guard.py --mode enforce
gate python3 scripts/quality/slint-relative-time-guard.py --self-test
gate python3 scripts/quality/slint-relative-time-guard.py --mode enforce
gate python3 scripts/quality/slint-profile-source-guard.py --self-test
gate python3 scripts/quality/slint-profile-source-guard.py --mode enforce
gate python3 scripts/quality/capability-reason-localization-guard.py --self-test
gate python3 scripts/quality/capability-reason-localization-guard.py
gate python3 scripts/quality/platform-evidence-guard.py --self-test
gate python3 scripts/quality/platform-evidence-guard.py --mode enforce
gate python3 scripts/platform/build-evidence-matrix.py --check
gate python3 scripts/quality/implicit-state-audit.py --self-test
gate python3 scripts/quality/implicit-state-audit.py --mode enforce
gate python3 scripts/quality/iced-connection-view-guard.py --self-test
gate python3 scripts/quality/iced-connection-view-guard.py --mode enforce
gate python3 -m unittest discover -s scripts/parity -p 'test_*.py'
gate python3 scripts/parity/resolve_surface_evidence.py --structure-only --report-json target/parity/structure.json

# ---------------------------------------------------------------------------
# Ecosystem and release pipeline gates (PUB-006.7, PUB-006.9, PUB-006.10,
# PUB-006.11). Every gate below is offline: the structure gate never
# compiles, never downloads and never starts a Core.
# ---------------------------------------------------------------------------
gate python3 scripts/quality/examples-guard.py --self-test
gate python3 scripts/quality/examples-guard.py --mode structure
gate python3 scripts/quality/sandbox-chaos-guard.py --self-test
gate python3 scripts/quality/sandbox-chaos-guard.py --mode structure
gate python3 scripts/quality/semver-compat-guard.py --self-test
gate python3 scripts/quality/semver-compat-guard.py --mode check
gate python3 scripts/quality/semver-compat-guard.py --mode negative
gate python3 scripts/quality/cross-build-guard.py --self-test
gate python3 scripts/quality/cross-build-guard.py --mode enforce
gate python3 scripts/quality/journey-rehearsal-guard.py --self-test
gate python3 scripts/quality/journey-rehearsal-guard.py --mode enforce

# ---------------------------------------------------------------------------
# Engineering constraint gates (dependency policy, toolchain pin, unsafe
# traceability, panic boundaries, library stdout, build scripts, feature
# matrix). Every gate below is offline: it reads TOML, sources and scripts, it
# never compiles, never downloads and never starts a Core.
# ---------------------------------------------------------------------------
gate python3 scripts/quality/dependency-policy-guard.py --self-test
gate python3 scripts/quality/dependency-policy-guard.py --mode enforce
gate python3 scripts/quality/toolchain-consistency-guard.py --self-test
gate python3 scripts/quality/toolchain-consistency-guard.py --mode enforce
gate python3 scripts/quality/lint-inheritance-guard.py --self-test
gate python3 scripts/quality/lint-inheritance-guard.py --mode enforce
gate python3 scripts/quality/unsafe-traceability-guard.py --self-test
gate python3 scripts/quality/unsafe-traceability-guard.py --mode enforce
gate python3 scripts/quality/panic-boundary-guard.py --self-test
gate python3 scripts/quality/panic-boundary-guard.py --mode enforce
gate python3 scripts/quality/stdout-discipline-guard.py --self-test
gate python3 scripts/quality/stdout-discipline-guard.py --mode enforce
gate python3 scripts/quality/build-surface-guard.py --self-test
gate python3 scripts/quality/build-surface-guard.py --mode enforce
gate python3 scripts/quality/feature-matrix-guard.py --self-test
gate python3 scripts/quality/feature-matrix-guard.py --mode enforce

# ---------------------------------------------------------------------------
# Summary: the whole batch reports once, and the ledger decides whether the
# failing set is the declared one.
# ---------------------------------------------------------------------------
printf 'structure gate: %d leg(s) executed, %d failed\n' "$GATE_INDEX" "$FAILED_LEGS" >&2
if [ "$FAILED_LEGS" -ne 0 ]; then
  cat "$GATE_FAILURES" >&2
fi
python3 scripts/quality/structure-gate-guard.py --mode enforce --failures "$GATE_FAILURES" \
  || FAILED_LEGS=$((FAILED_LEGS + 1))
exit "$((FAILED_LEGS > 0 ? 1 : 0))"
