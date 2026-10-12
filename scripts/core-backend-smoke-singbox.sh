#!/usr/bin/env bash
set -Eeuo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
evidence_lock="$repo_root/packaging/sing-box-evidence-assets.json"

for command_name in cargo curl python3 sha256sum tar mktemp awk rg; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf '[FAIL] required command is unavailable: %s\n' "$command_name" >&2
        exit 1
    fi
done

if ! cargo nextest --version >/dev/null 2>&1; then
    printf '[FAIL] cargo-nextest is unavailable; real adapter evidence requires nextest\n' >&2
    exit 1
fi

if [[ "$(uname -s):$(uname -m)" != Linux:x86_64 ]]; then
    printf '[FAIL] real sing-box smoke currently requires Linux x86_64; host is %s/%s\n' \
        "$(uname -s)" "$(uname -m)" >&2
    exit 1
fi

asset_record="$(python3 - "$evidence_lock" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    manifest = json.load(source)
if manifest.get("schema_version") != 2 or manifest.get("engine") != "sing-box":
    raise SystemExit("sing-box evidence lock must use schema 2 and name sing-box")
target = "x86_64-unknown-linux-gnu"
release = manifest["release"]
matches = [
    asset for asset in manifest["assets"]
    if asset["binary_target"] == target
    and asset["target"] == target
    and asset["adapter_revision"] == 1
    and asset["version"] == release["version"] == "v1.14.3"
]
if len(matches) != 1:
    raise SystemExit(
        "sing-box evidence lock must contain one v1.14.3 asset for "
        "x86_64-unknown-linux-gnu adapter revision 1"
    )
asset = matches[0]
print("\t".join((
    release["version"],
    asset["archive"],
    asset["archive_sha256"],
    asset["download_url"],
)))
PY
)"
IFS=$'\t' read -r expected_version archive expected_digest download_url <<<"$asset_record"

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/music-frog-singbox-smoke.XXXXXX")"
cleanup() {
    local exit_status=$?
    trap - EXIT INT TERM
    rm -rf -- "$run_dir"
    exit "$exit_status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

archive_path="$run_dir/$archive"
if ! curl --fail --location --retry 2 --silent --show-error \
    --output "$archive_path" "$download_url"; then
    printf '[FAIL] official sing-box asset download failed for %s\n' "$expected_version" >&2
    exit 1
fi
actual_digest="$(sha256sum "$archive_path" | awk '{print $1}')"
if [[ "$actual_digest" != "$expected_digest" ]]; then
    printf '[FAIL] sing-box archive digest mismatch for %s\n' "$expected_version" >&2
    printf '       expected: %s\n       actual:   %s\n' "$expected_digest" "$actual_digest" >&2
    exit 1
fi

mkdir -p "$run_dir/extracted"
tar -xzf "$archive_path" -C "$run_dir/extracted"
archive_dir="${archive%.tar.gz}"
binary_path="$run_dir/extracted/$archive_dir/sing-box"
if [[ ! -x "$binary_path" ]]; then
    printf '[FAIL] expected binary is absent from the locked sing-box archive\n' >&2
    exit 1
fi
if ! "$binary_path" version | head -n 1 | rg -Fx "sing-box version ${expected_version#v}" >/dev/null; then
    printf '[FAIL] sing-box binary version does not match its asset lock\n' >&2
    exit 1
fi
printf '[PASS] official sing-box archive and binary version verified: %s\n' "$expected_version"

SINGBOX_REAL_BINARY="$binary_path" \
SINGBOX_REAL_VERSION="${expected_version#v}" \
    cargo nextest run --offline --locked -p infiltrator-composition \
        --test singbox_real_process_smoke --run-ignored only
