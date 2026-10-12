#!/usr/bin/env bash
set -Eeuo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
mihomo_lock="$repo_root/packaging/mihomo-evidence-assets.json"
singbox_lock="$repo_root/packaging/sing-box-evidence-assets.json"

for command_name in cargo curl python3 sha256sum gzip tar mktemp uname awk; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf '[FAIL] required command is unavailable: %s\n' "$command_name" >&2
        exit 1
    fi
done

if ! cargo nextest --version >/dev/null 2>&1; then
    printf '[FAIL] cargo-nextest is unavailable; real conformance requires nextest\n' >&2
    exit 1
fi

if [[ "$(uname -s):$(uname -m)" != Linux:x86_64 && "$(uname -s):$(uname -m)" != Linux:amd64 ]]; then
    printf '[FAIL] real conformance assets require Linux amd64; host is %s/%s\n' \
        "$(uname -s)" "$(uname -m)" >&2
    exit 1
fi

temporary_root="${TMPDIR:-/tmp}"
run_dir="$(mktemp -d "$temporary_root/music-frog-core-conformance.XXXXXX")"
cleanup() {
    local exit_status=$?
    trap - EXIT INT TERM
    rm -rf -- "$run_dir"
    exit "$exit_status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

mihomo_record="$(python3 - "$mihomo_lock" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    lock = json.load(source)
if lock.get("schema_version") != 2:
    raise SystemExit("Mihomo evidence lock must use schema version 2")
matches = [
    item for item in lock["assets"]
    if item["version"] == "v1.19.32"
    and item["binary_target"] == "x86_64-unknown-linux-gnu"
    and item["adapter_revision"] == 1
]
if len(matches) != 1:
    raise SystemExit(
        "Mihomo evidence lock must contain one v1.19.32 asset for "
        "x86_64-unknown-linux-gnu adapter revision 1"
    )
asset = matches[0]
download_url = asset["release_url"].replace("/tag/", "/download/") + "/" + asset["archive"]
print("\t".join((
    asset["version"], asset["archive"], asset["archive_sha256"],
    asset["binary"], asset["binary_sha256"], download_url,
)))
PY
)"
IFS=$'\t' read -r mihomo_version mihomo_archive mihomo_archive_sha256 mihomo_binary_name mihomo_binary_sha256 mihomo_url <<<"$mihomo_record"
mihomo_archive_path="$run_dir/$mihomo_archive"
mihomo_binary_path="$run_dir/$mihomo_binary_name"
curl --fail --location --retry 2 --silent --show-error \
    --output "$mihomo_archive_path" "$mihomo_url"
actual_mihomo_archive_sha256="$(sha256sum "$mihomo_archive_path" | awk '{print $1}')"
if [[ "$actual_mihomo_archive_sha256" != "$mihomo_archive_sha256" ]]; then
    printf '[FAIL] Mihomo archive digest mismatch for %s\n' "$mihomo_version" >&2
    exit 1
fi
gzip -dc "$mihomo_archive_path" >"$mihomo_binary_path"
chmod 700 "$mihomo_binary_path"
actual_mihomo_binary_sha256="$(sha256sum "$mihomo_binary_path" | awk '{print $1}')"
if [[ "$actual_mihomo_binary_sha256" != "$mihomo_binary_sha256" ]]; then
    printf '[FAIL] Mihomo extracted binary digest mismatch for %s\n' "$mihomo_version" >&2
    exit 1
fi
printf '[PASS] locked Mihomo asset and binary SHA-256 verified: %s\n' "$mihomo_version"

singbox_record="$(python3 - "$singbox_lock" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    lock = json.load(source)
if lock.get("schema_version") != 2 or lock.get("engine") != "sing-box":
    raise SystemExit("sing-box evidence lock must use schema 2 and name sing-box")
target = "x86_64-unknown-linux-gnu"
matches = [
    item for item in lock["assets"]
    if item["binary_target"] == target
    and item["target"] == target
    and item["adapter_revision"] == 1
    and item["version"] == lock["release"]["version"] == "v1.14.3"
]
if len(matches) != 1:
    raise SystemExit(
        "sing-box evidence lock must contain one v1.14.3 asset for "
        "x86_64-unknown-linux-gnu adapter revision 1"
    )
asset = matches[0]
print("\t".join((
    lock["release"]["version"], asset["archive"], asset["archive_sha256"],
    asset["download_url"],
)))
PY
)"
IFS=$'\t' read -r singbox_version singbox_archive singbox_archive_sha256 singbox_url <<<"$singbox_record"
singbox_archive_path="$run_dir/$singbox_archive"
curl --fail --location --retry 2 --silent --show-error \
    --output "$singbox_archive_path" "$singbox_url"
actual_singbox_archive_sha256="$(sha256sum "$singbox_archive_path" | awk '{print $1}')"
if [[ "$actual_singbox_archive_sha256" != "$singbox_archive_sha256" ]]; then
    printf '[FAIL] sing-box archive digest mismatch for %s\n' "$singbox_version" >&2
    exit 1
fi
mkdir -p "$run_dir/singbox"
tar -xzf "$singbox_archive_path" -C "$run_dir/singbox"
singbox_binary_path="$run_dir/singbox/${singbox_archive%.tar.gz}/sing-box"
if [[ ! -x "$singbox_binary_path" ]]; then
    printf '[FAIL] expected sing-box executable is absent from the locked archive\n' >&2
    exit 1
fi
printf '[PASS] locked sing-box archive SHA-256 verified: %s\n' "$singbox_version"

printf '[INFO] running shared engine-neutral fake/contract conformance cases\n'
cargo nextest run --offline --locked -p infiltrator-core --test backend_conformance

printf '[INFO] running shared typed assertions against both locked real processes\n'
MIHOMO_REAL_BINARY="$mihomo_binary_path" \
MIHOMO_REAL_VERSION="$mihomo_version" \
SINGBOX_REAL_BINARY="$singbox_binary_path" \
SINGBOX_REAL_VERSION="${singbox_version#v}" \
    cargo nextest run --offline --locked -p infiltrator-core \
        --test backend_conformance --run-ignored only

printf '[INFO] running the production durable Mihomo/sing-box owner-switch smoke\n'
MIHOMO_REAL_BINARY="$mihomo_binary_path" \
MIHOMO_REAL_VERSION="$mihomo_version" \
SINGBOX_REAL_BINARY="$singbox_binary_path" \
SINGBOX_REAL_VERSION="${singbox_version#v}" \
    cargo nextest run --offline --locked -p infiltrator-composition \
        --test desktop_owner_switch_real_process_smoke --run-ignored only

printf '[PASS] real-process conformance and durable Mihomo/sing-box owner-switch smoke completed\n'
