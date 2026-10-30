#!/usr/bin/env bash
# Real engine conformance smoke: the shared engine-neutral conformance cases
# and the two driver real-binary proofs against the locked release assets.
#
# What this entry point produces
# ------------------------------
# * The locked Mihomo and sing-box release assets, both verified against their
#   immutable asset locks.
# * The shared engine-neutral conformance cases: the fake/contract cases in
#   the ordinary gate and the same typed assertions against both real
#   processes.
# * The durable Mihomo/sing-box owner-switch smoke.
# * The two driver real-binary proofs: the named test targets
#   `real_process_evidence` of `mihomo-driver` and `singbox-driver` prove the
#   controller handshake, the typed node speed test, a real traffic flow
#   through the engine's own listener and a configuration write with its
#   readback. Their environment contracts live in the driver crates; the
#   per-engine entry points read the environment key names from those
#   contracts.
# * One machine-readable summary line (`MUSIC_FROG_SMOKE_SUMMARY <json>`) that
#   names what was proven, which binary identity and digest were used, what
#   was skipped and why.
#
# Exit codes
# ----------
# 0  the summary reports `proven`, `incomplete` or `environment_absent`:
#    every step that could run ran and passed, and the steps that could not
#    run are listed in `skipped` with their reason.
# 1  a real defect: an assertion failed, a digest mismatched, or the summary
#    could not be produced.
# 2  a usage error.
#
# Usage
# -----
#   scripts/core-backend-smoke-conformance.sh [--summary-file PATH]
set -Eeuo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
mihomo_lock="$repo_root/packaging/mihomo-evidence-assets.json"
singbox_lock="$repo_root/packaging/sing-box-evidence-assets.json"

summary_file=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --summary-file)
            shift
            summary_file="${1:-}"
            ;;
        *)
            printf '[FAIL] usage: %s [--summary-file PATH]\n' "${BASH_SOURCE[0]}" >&2
            exit 2
            ;;
    esac
    shift
done

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

# --- machine-readable summary -------------------------------------------------
# The summary is the single machine-readable artifact of this entry point. It
# names what was proven, which binary identity and digest were used, what was
# skipped and why, and what is a defect. The exit code follows the summary:
# `proven`, `incomplete` and `environment_absent` exit 0, `defect` exits 1.
summary_outcome="proven"
summary_proven=""
summary_not_proven=""
summary_skipped=""
summary_defects=""
summary_receipts=""
summary_binaries="[]"

summary_add() {
    # summary_add <proven|not_proven|skipped|defects> <item> [reason]
    local list=$1
    local item=$2
    local reason=${3-}
    local entry="$item"
    if [[ -n "$reason" ]]; then
        entry="$item"$'\t'"$reason"
    fi
    case "$list" in
        proven) summary_proven="${summary_proven}${entry}"$'\n' ;;
        not_proven) summary_not_proven="${summary_not_proven}${entry}"$'\n' ;;
        skipped) summary_skipped="${summary_skipped}${entry}"$'\n' ;;
        defects) summary_defects="${summary_defects}${entry}"$'\n' ;;
    esac
}

emit_summary() {
    local status=$1
    local outcome="$summary_outcome"
    if [[ "$status" -ne 0 ]]; then
        outcome="defect"
    fi
    MUSIC_FROG_SMOKE_ENGINE="mihomo+sing-box" \
    MUSIC_FROG_SMOKE_OUTCOME="$outcome" \
    MUSIC_FROG_SMOKE_PROVEN="$summary_proven" \
    MUSIC_FROG_SMOKE_NOT_PROVEN="$summary_not_proven" \
    MUSIC_FROG_SMOKE_SKIPPED="$summary_skipped" \
    MUSIC_FROG_SMOKE_DEFECTS="$summary_defects" \
    MUSIC_FROG_SMOKE_RECEIPTS="$summary_receipts" \
    MUSIC_FROG_SMOKE_BINARIES="$summary_binaries" \
    MUSIC_FROG_SMOKE_SUMMARY_FILE="$summary_file" \
    python3 - <<'PY'
import json
import os


def entries(name):
    result = []
    for line in os.environ.get(name, "").split("\n"):
        if not line:
            continue
        item, _, reason = line.partition("\t")
        entry = {"item": item}
        if reason:
            entry["reason"] = reason
        result.append(entry)
    return result


payload = {
    "schema": "music-frog.core-backend-smoke/1",
    "engine": os.environ.get("MUSIC_FROG_SMOKE_ENGINE", ""),
    "outcome": os.environ.get("MUSIC_FROG_SMOKE_OUTCOME", "defect"),
    "binaries": json.loads(os.environ.get("MUSIC_FROG_SMOKE_BINARIES", "[]")),
    "proven": entries("MUSIC_FROG_SMOKE_PROVEN"),
    "not_proven": entries("MUSIC_FROG_SMOKE_NOT_PROVEN"),
    "skipped": entries("MUSIC_FROG_SMOKE_SKIPPED"),
    "defects": entries("MUSIC_FROG_SMOKE_DEFECTS"),
    "driver_receipts": [
        receipt
        for line in os.environ.get("MUSIC_FROG_SMOKE_RECEIPTS", "").split("\n")
        if line
        for receipt in (
            json.loads(line) if isinstance(json.loads(line), list) else [json.loads(line)]
        )
    ],
}
text = json.dumps(payload, sort_keys=True)
print("MUSIC_FROG_SMOKE_SUMMARY " + text)
path = os.environ.get("MUSIC_FROG_SMOKE_SUMMARY_FILE", "")
if path:
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text + "\n")
PY
}

temporary_root="${TMPDIR:-/tmp}"
run_dir="$(mktemp -d "$temporary_root/music-frog-core-conformance.XXXXXX")"
cleanup() {
    local exit_status=$?
    trap - EXIT INT TERM
    if [[ "$exit_status" -ne 0 ]]; then
        summary_add defects "conformance_run" \
            "the conformance run stopped with exit status ${exit_status}"
    fi
    if [[ -n "$run_dir" && -d "$run_dir" ]]; then
        rm -rf -- "$run_dir"
    fi
    if ! emit_summary "$exit_status"; then
        exit_status=1
    fi
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
    if item["version"] == "v1.19.33"
    and item["binary_target"] == "x86_64-unknown-linux-gnu"
]
if len(matches) != 1:
    raise SystemExit(
        "Mihomo evidence lock must contain one v1.19.33 asset for "
        "x86_64-unknown-linux-gnu"
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
    asset for asset in lock["assets"]
    if asset["binary_target"] == target
    and asset["target"] == target
    and asset["adapter_revision"] == 1
    and asset["version"] == lock["release"]["version"] == "v1.14.3"
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

summary_binaries="$(python3 - "$mihomo_binary_path" "$actual_mihomo_binary_sha256" \
    "$mihomo_version" "$singbox_binary_path" "$singbox_archive_sha256" "$singbox_version" <<'PY'
import json
import sys

mihomo_path, mihomo_digest, mihomo_version, singbox_path, singbox_digest, singbox_version = sys.argv[1:7]
print(json.dumps([
    {
        "engine": "mihomo",
        "path": mihomo_path,
        "sha256": mihomo_digest,
        "version": mihomo_version,
        "source": "locked-release-asset",
    },
    {
        "engine": "sing-box",
        "path": singbox_path,
        "sha256": singbox_digest,
        "version": singbox_version,
        "source": "locked-release-asset",
    },
], sort_keys=True))
PY
)"

printf '[INFO] running shared engine-neutral fake/contract conformance cases\n'
cargo nextest run --offline --locked -p infiltrator-core --test backend_conformance
summary_add proven "shared_conformance_fake" \
    "the shared engine-neutral conformance cases passed against the fakes"

printf '[INFO] running shared typed assertions against both locked real processes\n'
MIHOMO_REAL_BINARY="$mihomo_binary_path" \
MIHOMO_REAL_VERSION="$mihomo_version" \
SINGBOX_REAL_BINARY="$singbox_binary_path" \
SINGBOX_REAL_VERSION="${singbox_version#v}" \
    cargo nextest run --offline --locked -p infiltrator-core \
        --test backend_conformance --run-ignored only
summary_add proven "shared_conformance_real" \
    "the shared typed assertions passed against both locked real processes"

printf '[INFO] running the production durable Mihomo/sing-box owner-switch smoke\n'
MIHOMO_REAL_BINARY="$mihomo_binary_path" \
MIHOMO_REAL_VERSION="$mihomo_version" \
SINGBOX_REAL_BINARY="$singbox_binary_path" \
SINGBOX_REAL_VERSION="${singbox_version#v}" \
    cargo nextest run --offline --locked -p infiltrator-composition \
        --test desktop_owner_switch_real_process_smoke --run-ignored only
summary_add proven "owner_switch_real" \
    "the durable Mihomo/sing-box owner-switch smoke passed against both locked real processes"

# The two driver proofs own their own environment contracts, so the per-engine
# entry points read the contract and write their own machine-readable receipt.
printf '[INFO] running the Mihomo driver real-binary evidence\n'
mihomo_driver_summary="$run_dir/mihomo-driver-summary.json"
if ! MIHOMO_REAL_BINARY="$mihomo_binary_path" \
    bash "$script_dir/core-backend-smoke-mihomo.sh" --driver-evidence-only \
        --summary-file "$mihomo_driver_summary"; then
    summary_add defects "driver_real_process_evidence" \
        "the Mihomo driver real-binary evidence failed"
    exit 1
fi

printf '[INFO] running the sing-box driver real-binary evidence\n'
singbox_driver_summary="$run_dir/singbox-driver-summary.json"
if ! SINGBOX_REAL_BINARY="$singbox_binary_path" \
    bash "$script_dir/core-backend-smoke-singbox.sh" --driver-evidence-only \
        --summary-file "$singbox_driver_summary"; then
    summary_add defects "driver_real_process_evidence" \
        "the sing-box driver real-binary evidence failed"
    exit 1
fi

python3 - "$mihomo_driver_summary" "$singbox_driver_summary" "$run_dir" <<'PY'
import json
import sys

mihomo_path, singbox_path, out_dir = sys.argv[1:4]
lists = {"proven": [], "not_proven": [], "skipped": [], "defects": []}
receipts = []
for path in (mihomo_path, singbox_path):
    with open(path, encoding="utf-8") as source:
        payload = json.load(source)
    engine = payload["engine"]
    receipts.extend(payload["driver_receipts"])
    for name in lists:
        for entry in payload[name]:
            lists[name].append(f"{engine}:{entry['item']}\t{entry.get('reason', '')}")
    if payload["outcome"] == "incomplete":
        lists["not_proven"].append(f"{engine}:driver_outcome\tincomplete")
    if payload["outcome"] == "environment_absent":
        lists["skipped"].append(f"{engine}:driver_outcome\tenvironment_absent")
for name, entries in lists.items():
    with open(f"{out_dir}/{name}.txt", "w", encoding="utf-8") as handle:
        handle.write("\n".join(entries))
with open(f"{out_dir}/receipts.json", "w", encoding="utf-8") as handle:
    json.dump(receipts, handle, sort_keys=True)
PY

for list_name in proven not_proven skipped defects; do
    list_content="$(cat "$run_dir/$list_name.txt" 2>/dev/null || true)"
    if [[ -z "$list_content" ]]; then
        continue
    fi
    case "$list_name" in
        proven) summary_proven="${summary_proven}${list_content}"$'\n' ;;
        not_proven) summary_not_proven="${summary_not_proven}${list_content}"$'\n' ;;
        skipped) summary_skipped="${summary_skipped}${list_content}"$'\n' ;;
        defects) summary_defects="${summary_defects}${list_content}"$'\n' ;;
    esac
done
summary_receipts="$(cat "$run_dir/receipts.json")"
if grep -q "not_proven" "$run_dir/not_proven.txt" 2>/dev/null; then
    summary_outcome="incomplete"
fi
if grep -q "environment_absent" "$run_dir/skipped.txt" 2>/dev/null; then
    summary_outcome="environment_absent"
fi

printf '[PASS] real-process conformance and durable Mihomo/sing-box owner-switch smoke completed\n'
