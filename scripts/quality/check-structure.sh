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
python3 scripts/quality/bevy_shader_guard.py --mode enforce
python3 scripts/quality/line-guard.py --self-test
python3 scripts/quality/line-guard.py --mode enforce
python3 scripts/quality/kt-line-guard.py --self-test
python3 scripts/quality/kt-line-guard.py --mode enforce
python3 scripts/quality/doc-governance-guard.py --self-test
python3 scripts/quality/doc-governance-guard.py --mode enforce
python3 scripts/quality/test-layout-guard.py --mode enforce
python3 scripts/quality/typed-state-guard.py --self-test
python3 scripts/quality/typed-state-guard.py --mode enforce
python3 scripts/quality/retired-surface-guard.py
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
python3 -m unittest discover -s scripts/parity -p 'test_*.py'
python3 scripts/parity/resolve_surface_evidence.py --structure-only --report-json target/parity/structure.json
