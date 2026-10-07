#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"
bash scripts/check-test-policy.sh
python3 scripts/quality/import-guard.py --self-test
python3 scripts/quality/import-guard.py --mode enforce
python3 scripts/quality/core-boundary-guard.py --mode enforce
python3 scripts/quality/bevy_bsn_guard.py --mode enforce
python3 scripts/quality/bevy_shader_guard.py --mode enforce
python3 scripts/quality/line-guard.py --self-test
python3 scripts/quality/line-guard.py --mode enforce
python3 scripts/quality/test-layout-guard.py --mode enforce
python3 scripts/quality/verify-packaging.py
python3 -m unittest discover -s scripts/packaging_tests -p 'test_*.py'
python3 -m unittest discover -s scripts/quality/tests -p 'test_*.py'
python3 scripts/quality/i18n-guard.py --mode enforce
python3 scripts/quality/android-manifest-guard.py --mode enforce
python3 -m unittest discover -s scripts/parity -p 'test_*.py'
python3 scripts/parity/resolve_surface_evidence.py --structure-only --report-json target/parity/structure.json
