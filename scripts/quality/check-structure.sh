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
python3 scripts/quality/retired-surface-guard.py
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
python3 scripts/quality/bevy-desktop-host-boundary-guard.py --self-test
python3 scripts/quality/bevy-desktop-host-boundary-guard.py
python3 scripts/quality/android-owner-client-guard.py --self-test
python3 scripts/quality/android-owner-client-guard.py
python3 scripts/quality/android-vpn-lifecycle-guard.py --self-test
python3 scripts/quality/android-vpn-lifecycle-guard.py
python3 scripts/quality/android-manifest-contract-guard.py
python3 scripts/quality/android-so-combination-guard.py --self-test
python3 scripts/quality/android-so-combination-guard.py
python3 scripts/quality/verify-packaging.py
python3 -m unittest discover -s scripts/packaging_tests -p 'test_*.py'
python3 -m unittest discover -s scripts/quality/tests -p 'test_*.py'
python3 scripts/quality/i18n-guard.py --mode enforce
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
python3 scripts/quality/implicit-state-audit.py --self-test
python3 scripts/quality/implicit-state-audit.py --mode enforce
python3 -m unittest discover -s scripts/parity -p 'test_*.py'
python3 scripts/parity/resolve_surface_evidence.py --structure-only --report-json target/parity/structure.json
