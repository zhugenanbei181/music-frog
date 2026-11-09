#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"
bash scripts/check-test-policy.sh
python3 scripts/quality/import-guard.py --self-test
python3 scripts/quality/import-guard.py --mode enforce
python3 scripts/quality/slint-source-import-guard.py
python3 scripts/quality/slint-syntax-guard.py --self-test
python3 scripts/quality/slint-syntax-guard.py --mode enforce
python3 scripts/quality/slint-typography-role-guard.py --self-test
python3 scripts/quality/slint-typography-role-guard.py --mode enforce
python3 scripts/quality/slint-dead-input-guard.py --self-test
python3 scripts/quality/slint-dead-input-guard.py --mode enforce
python3 scripts/quality/core-boundary-guard.py --mode enforce
python3 scripts/quality/adapter-operation-declarations-guard.py
python3 scripts/quality/workspace-pin-guard.py --mode enforce
python3 scripts/quality/bevy_bsn_guard.py --mode enforce
python3 scripts/quality/bevy_rem_guard.py --mode enforce
# Bevy 场景律法（BSN 唯一构造 + rem 唯一 authored 长度）的执行器随共享包
# zhuge-ui-kit 一起发布；此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
python3 scripts/quality/bevy_scene_gate.py \
  --root crates/infiltrator-bevy-ui/src \
  --root crates/infiltrator-bevy-widgets/src \
  --baseline scripts/quality/bevy_scene_ratchet.json
python3 -m unittest discover -s scripts/quality/tests -p 'test_bevy_guard_owner_binding.py'
# 带宽算术唯一律（最宽带只有一个 owner）的执行器随共享包 zhuge-ui-core 一起发布；
# 此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
python3 scripts/quality/band_arithmetic_gate.py \
  --root crates \
  --baseline scripts/quality/band_arithmetic_ratchet.json
python3 scripts/quality/test_band_arithmetic_gate.py
# 区间归一化唯一律（进入单位区间的投影只有一个 owner）的执行器随共享包 zhuge-ui-core
# 一起发布；此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
python3 scripts/quality/interval_normalization_gate.py \
  --root crates \
  --baseline scripts/quality/interval_normalization_ratchet.json
python3 scripts/quality/test_interval_normalization_gate.py
# 标量混合唯一律（`A + (B - A) * t` 只有一个 owner）的执行器随共享包 zhuge-ui-core
# 一起发布；此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
python3 scripts/quality/scalar_mix_gate.py \
  --root crates \
  --baseline scripts/quality/scalar_mix_ratchet.json
python3 scripts/quality/test_scalar_mix_gate.py
# 缓动斜坡与 Bézier 求解唯一律（`smoothstep` 形状与「按进度解曲线参数」各只有一个 owner）的执行器
# 随共享包 zhuge-ui-core 一起发布；此处是同一份文件的拷贝，台账记录其摘要，改动必须同步。
python3 scripts/quality/easing_owner_gate.py \
  --root crates \
  --baseline scripts/quality/easing_owner_ratchet.json
python3 scripts/quality/test_easing_owner_gate.py
python3 scripts/quality/bevy_shader_guard.py --mode enforce
python3 scripts/quality/line-guard.py --self-test
python3 scripts/quality/line-guard.py --mode enforce
python3 scripts/quality/kt-line-guard.py --self-test
python3 scripts/quality/kt-line-guard.py --mode enforce
python3 scripts/quality/doc-governance-guard.py --self-test
python3 scripts/quality/doc-governance-guard.py --mode enforce
python3 scripts/quality/test-layout-guard.py --mode enforce
python3 scripts/quality/mod-rs-guard.py --self-test
python3 scripts/quality/mod-rs-guard.py --mode enforce
python3 scripts/quality/typed-state-guard.py --self-test
python3 scripts/quality/typed-state-guard.py --mode enforce
python3 scripts/quality/screenshot-identity-guard.py --self-test
python3 scripts/quality/screenshot-identity-guard.py --mode enforce
python3 scripts/quality/receipt-freshness-guard.py --self-test
python3 scripts/quality/receipt-freshness-guard.py --mode enforce
python3 scripts/quality/retired-surface-guard.py
python3 -m unittest discover -s scripts/quality/tests -p 'test_easing_owner_tests_mutation.py'
python3 scripts/quality/lurker-neutrality-guard.py --self-test
python3 scripts/quality/lurker-neutrality-guard.py
python3 scripts/quality/publish-order-guard.py --self-test
python3 scripts/quality/publish-order-guard.py
python3 scripts/quality/rustdoc-guard.py --self-test
python3 scripts/quality/rustdoc-guard.py --mode structure
python3 scripts/quality/rustdoc-guard.py --mode docs
python3 scripts/quality/neutral-release-guard.py --self-test
python3 scripts/quality/neutral-release-guard.py --mode structure
python3 scripts/quality/closure-questions-guard.py --self-test
python3 scripts/quality/closure-questions-guard.py
python3 scripts/quality/android-bevy-driver-dependency-guard.py --self-test
python3 scripts/quality/android-bevy-driver-dependency-guard.py
python3 scripts/quality/mobile-cfg-discipline-guard.py --self-test
python3 scripts/quality/mobile-cfg-discipline-guard.py --mode enforce
python3 scripts/quality/bevy-desktop-host-boundary-guard.py --self-test
python3 scripts/quality/bevy-desktop-host-boundary-guard.py
python3 scripts/quality/android-owner-client-guard.py --self-test
python3 scripts/quality/android-owner-client-guard.py
python3 scripts/quality/android-vpn-lifecycle-guard.py --self-test
python3 scripts/quality/android-vpn-lifecycle-guard.py
python3 scripts/quality/android-manifest-contract-guard.py
python3 scripts/quality/android-architecture-redline-guard.py --self-test
python3 scripts/quality/android-architecture-redline-guard.py --mode enforce
python3 scripts/quality/android-so-combination-guard.py --self-test
python3 scripts/quality/android-so-combination-guard.py
python3 scripts/quality/real-process-evidence-guard.py --self-test
python3 scripts/quality/real-process-evidence-guard.py
python3 scripts/quality/android-interaction-receipt-guard.py --self-test
python3 scripts/quality/android-interaction-receipt-guard.py --mode enforce
python3 scripts/quality/verify-packaging.py
python3 -m unittest discover -s scripts/packaging_tests -p 'test_*.py'
python3 -m unittest discover -s scripts/quality/tests -p 'test_*.py'
python3 -m unittest discover -s scripts/quality/tests -p 'test_slint_mobile_chrome_guard.py'
python3 scripts/quality/i18n-guard.py --mode enforce
python3 scripts/quality/design-resource-authority.py --self-test
python3 scripts/quality/design-resource-authority.py --skip-exporter
python3 scripts/quality/shared-copy-guard.py --mode enforce
python3 scripts/quality/shared-copy-guard.py --self-test
python3 scripts/quality/scrollbar-contract-guard.py --mode enforce
python3 scripts/quality/scrollbar-contract-guard.py --self-test
python3 scripts/quality/slint-mobile-chrome-guard.py --mode enforce
python3 scripts/quality/slint-mobile-chrome-guard.py --self-test
python3 scripts/quality/core-session-guard.py --mode enforce
python3 scripts/quality/core-session-guard.py --self-test
python3 scripts/quality/slint-fact-row-guard.py --mode enforce
python3 scripts/quality/slint-fact-row-guard.py --self-test
python3 scripts/quality/slint-padding-guard.py --mode enforce
python3 scripts/quality/slint-padding-guard.py --self-test
python3 scripts/quality/slint-dock-material-guard.py --mode enforce
python3 scripts/quality/slint-dock-material-guard.py --self-test
python3 scripts/quality/slint-surface-ladder-guard.py --mode enforce
python3 scripts/quality/slint-surface-ladder-guard.py --self-test
python3 scripts/quality/slint-viewport-tier-guard.py --mode enforce
python3 scripts/quality/slint-viewport-tier-guard.py --self-test
python3 scripts/quality/slint-list-page-guard.py --mode enforce
python3 scripts/quality/slint-list-page-guard.py --self-test
python3 scripts/parity/space_utilization.py --self-test
python3 scripts/parity/space_utilization.py --check-export
python3 scripts/parity/space_utilization.py --enforce
python3 scripts/quality/slint-workbench-row-guard.py --mode enforce
python3 scripts/quality/slint-workbench-row-guard.py --self-test
python3 scripts/quality/slint-selector-segment-guard.py --mode enforce
python3 scripts/quality/slint-selector-segment-guard.py --self-test
python3 scripts/quality/slint-dashboard-grid-guard.py --mode enforce
python3 scripts/quality/slint-dashboard-grid-guard.py --self-test
python3 scripts/quality/slint-action-label-guard.py --mode enforce
python3 scripts/quality/slint-action-label-guard.py --self-test
python3 scripts/quality/slint-rail-edge-dialect-guard.py --mode enforce
python3 scripts/quality/slint-rail-edge-dialect-guard.py --self-test
python3 scripts/quality/slint-sidebar-separator-guard.py --mode enforce
python3 scripts/quality/slint-sidebar-separator-guard.py --self-test
python3 scripts/quality/android-product-identity-guard.py
python3 scripts/quality/release-build-target-guard.py --self-test
python3 scripts/quality/release-build-target-guard.py
python3 scripts/quality/android-mihomo-assets-guard.py
python3 scripts/quality/apple-ci-deferral-guard.py
python3 scripts/quality/ci-trigger-guard.py
python3 scripts/quality/license-guard.py --self-test
python3 scripts/quality/license-guard.py --mode enforce
python3 scripts/quality/parity-coverage-guard.py --self-test
python3 scripts/quality/parity-coverage-guard.py --mode enforce
python3 scripts/quality/flclash-core-parity-guard.py --self-test
python3 scripts/quality/flclash-core-parity-guard.py --mode enforce
python3 scripts/quality/flclash-dashboard-widget-guard.py --self-test
python3 scripts/quality/flclash-dashboard-widget-guard.py --mode enforce
python3 scripts/quality/capability-reason-localization-guard.py --self-test
python3 scripts/quality/capability-reason-localization-guard.py
python3 scripts/quality/platform-evidence-guard.py --self-test
python3 scripts/quality/platform-evidence-guard.py --mode enforce
python3 scripts/platform/build-evidence-matrix.py --check
python3 scripts/quality/implicit-state-audit.py --self-test
python3 scripts/quality/implicit-state-audit.py --mode enforce
python3 -m unittest discover -s scripts/parity -p 'test_*.py'
python3 scripts/parity/resolve_surface_evidence.py --structure-only --report-json target/parity/structure.json

# ---------------------------------------------------------------------------
# Ecosystem and release pipeline gates (PUB-006.7, PUB-006.9, PUB-006.10,
# PUB-006.11). Every gate below is offline: the structure gate never
# compiles, never downloads and never starts a Core.
# ---------------------------------------------------------------------------
python3 scripts/quality/examples-guard.py --self-test
python3 scripts/quality/examples-guard.py --mode structure
python3 scripts/quality/sandbox-chaos-guard.py --self-test
python3 scripts/quality/sandbox-chaos-guard.py --mode structure
python3 scripts/quality/semver-compat-guard.py --self-test
python3 scripts/quality/semver-compat-guard.py --mode check
python3 scripts/quality/semver-compat-guard.py --mode negative
python3 scripts/quality/cross-build-guard.py --self-test
python3 scripts/quality/cross-build-guard.py --mode enforce
python3 scripts/quality/journey-rehearsal-guard.py --self-test
python3 scripts/quality/journey-rehearsal-guard.py --mode enforce

# ---------------------------------------------------------------------------
# Engineering constraint gates (dependency policy, toolchain pin, unsafe
# traceability, panic boundaries, library stdout, build scripts, feature
# matrix). Every gate below is offline: it reads TOML, sources and scripts, it
# never compiles, never downloads and never starts a Core.
# ---------------------------------------------------------------------------
python3 scripts/quality/dependency-policy-guard.py --self-test
python3 scripts/quality/dependency-policy-guard.py --mode enforce
python3 scripts/quality/toolchain-consistency-guard.py --self-test
python3 scripts/quality/toolchain-consistency-guard.py --mode enforce
python3 scripts/quality/lint-inheritance-guard.py --self-test
python3 scripts/quality/lint-inheritance-guard.py --mode enforce
python3 scripts/quality/unsafe-traceability-guard.py --self-test
python3 scripts/quality/unsafe-traceability-guard.py --mode enforce
python3 scripts/quality/panic-boundary-guard.py --self-test
python3 scripts/quality/panic-boundary-guard.py --mode enforce
python3 scripts/quality/stdout-discipline-guard.py --self-test
python3 scripts/quality/stdout-discipline-guard.py --mode enforce
python3 scripts/quality/build-surface-guard.py --self-test
python3 scripts/quality/build-surface-guard.py --mode enforce
python3 scripts/quality/feature-matrix-guard.py --self-test
python3 scripts/quality/feature-matrix-guard.py --mode enforce
