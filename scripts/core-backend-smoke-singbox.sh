#!/usr/bin/env bash
# Real sing-box evidence smoke: the locked release asset, the product-level
# typed smoke and the `singbox-driver` real-binary proof.
#
# What this entry point produces
# ------------------------------
# * The locked sing-box release archive, verified against
#   `packaging/sing-box-evidence-assets.json`, and the extracted binary.
# * The product-level typed smoke against the running engine.
# * The `singbox-driver` real-binary proof: the named test target
#   `real_process_evidence` proves the controller handshake, the typed node
#   speed test, a real traffic flow through the engine's own listener and a
#   configuration write with its readback. Its environment contract lives in
#   `crates/singbox-driver/tests/real_process_evidence/contract.rs`; this
#   script reads the environment key names from that file, so the script and
#   the test can never disagree about the contract.
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
#   scripts/core-backend-smoke-singbox.sh [--driver-evidence-only]
#                                           [--summary-file PATH]
#
# `SINGBOX_REAL_BINARY` may name an installed sing-box executable. When it is
# set the script uses that binary instead of downloading the locked release
# asset, and the summary records that the release archive was not verified.
set -Eeuo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
evidence_lock="$repo_root/packaging/sing-box-evidence-assets.json"
driver_contract="$repo_root/crates/singbox-driver/tests/real_process_evidence/contract.rs"

driver_only=false
summary_file=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --driver-evidence-only)
            driver_only=true
            ;;
        --summary-file)
            shift
            summary_file="${1:-}"
            ;;
        *)
            printf '[FAIL] usage: %s [--driver-evidence-only] [--summary-file PATH]\n' \
                "${BASH_SOURCE[0]}" >&2
            exit 2
            ;;
    esac
    shift
done

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

# --- machine-readable summary -------------------------------------------------
# The summary is the single machine-readable artifact of this entry point. It
# names what was proven, which binary identity and digest were used, what was
# skipped and why, and what is a defect. The exit code follows the summary:
# `proven`, `incomplete` and `environment_absent` exit 0, `defect` exits 1.
summary_outcome="proven"
summary_binary_path=""
summary_binary_sha256=""
summary_binary_version=""
summary_binary_source=""
summary_proven=""
summary_not_proven=""
summary_skipped=""
summary_defects=""
summary_receipts=""
product_smokes=true

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

free_loopback_port() {
    python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
    listener.bind(("127.0.0.1", 0))
    print(listener.getsockname()[1])
PY
}

emit_summary() {
    local status=$1
    local outcome="$summary_outcome"
    if [[ "$status" -ne 0 ]]; then
        outcome="defect"
    fi
    MUSIC_FROG_SMOKE_ENGINE="sing-box" \
    MUSIC_FROG_SMOKE_OUTCOME="$outcome" \
    MUSIC_FROG_SMOKE_BINARY_PATH="$summary_binary_path" \
    MUSIC_FROG_SMOKE_BINARY_SHA256="$summary_binary_sha256" \
    MUSIC_FROG_SMOKE_BINARY_VERSION="$summary_binary_version" \
    MUSIC_FROG_SMOKE_BINARY_SOURCE="$summary_binary_source" \
    MUSIC_FROG_SMOKE_PROVEN="$summary_proven" \
    MUSIC_FROG_SMOKE_NOT_PROVEN="$summary_not_proven" \
    MUSIC_FROG_SMOKE_SKIPPED="$summary_skipped" \
    MUSIC_FROG_SMOKE_DEFECTS="$summary_defects" \
    MUSIC_FROG_SMOKE_RECEIPTS="$summary_receipts" \
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
    "binary": {
        "path": os.environ.get("MUSIC_FROG_SMOKE_BINARY_PATH", ""),
        "sha256": os.environ.get("MUSIC_FROG_SMOKE_BINARY_SHA256", ""),
        "version": os.environ.get("MUSIC_FROG_SMOKE_BINARY_VERSION", ""),
        "source": os.environ.get("MUSIC_FROG_SMOKE_BINARY_SOURCE", ""),
    },
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

cleanup() {
    local exit_status=$?
    trap - EXIT INT TERM
    if [[ -n "$run_dir" && -d "$run_dir" ]]; then
        rm -rf -- "$run_dir"
    fi
    if ! emit_summary "$exit_status"; then
        exit_status=1
    fi
    exit "$exit_status"
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/music-frog-singbox-smoke.XXXXXX")"
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

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

if [[ -n "${SINGBOX_REAL_BINARY:-}" ]]; then
    binary_path="$SINGBOX_REAL_BINARY"
    if [[ ! -x "$binary_path" ]]; then
        printf '[FAIL] SINGBOX_REAL_BINARY does not name an executable: %s\n' "$binary_path" >&2
        exit 1
    fi
    actual_digest="$(sha256sum "$binary_path" | awk '{print $1}')"
    summary_add skipped "locked_release_asset" \
        "SINGBOX_REAL_BINARY was used; the locked release archive was not downloaded"
    summary_binary_source="environment"
else
    archive_path="$run_dir/$archive"
    if ! curl --fail --location --retry 2 --silent --show-error \
        --output "$archive_path" "$download_url"; then
        printf '[FAIL] official sing-box asset download failed for %s\n' "$expected_version" >&2
        exit 1
    fi
    actual_digest="$(sha256sum "$archive_path" | awk '{print $1}')"
    if [[ "$actual_digest" != "$expected_digest" ]]; then
        printf '[FAIL] sing-box archive digest mismatch for %s\n' "$expected_version" >&2
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
    printf '[PASS] official sing-box archive and binary version verified: %s\n' "$expected_version"
    summary_binary_source="locked-release-asset"
fi
if ! "$binary_path" version | head -n 1 | rg -Fx "sing-box version ${expected_version#v}" >/dev/null; then
    if [[ "$summary_binary_source" == "environment" ]]; then
        printf '[INFO] the environment-provided sing-box binary does not report the locked %s version; the product smoke of this leg is skipped\n' \
            "$expected_version"
        summary_add skipped "locked_product_smokes" \
            "the environment-provided binary does not report the locked ${expected_version} version"
        product_smokes=false
    else
        printf '[FAIL] sing-box binary version does not match its asset lock\n' >&2
        exit 1
    fi
fi
summary_binary_path="$binary_path"
summary_binary_sha256="$actual_digest"
summary_binary_version="$("$binary_path" version 2>/dev/null | head -n 1 | awk '{print $3}' || true)"

run_driver_evidence() {
    # The driver proof runs against its own isolated engine: its own
    # configuration document, its own loopback controller and its own
    # listeners. The environment key names are read from the typed contract of
    # the driver, so the script and the test share one contract.
    local driver_dir="$run_dir/driver-evidence"
    mkdir -m 700 -p "$driver_dir"

    local binary_env="" config_env="" instance_env="" controller_env="" secret_env=""
    local proxy_env="" fixture_env="" probe_env="" selector_env="" receipt_env=""
    local contract_map
    contract_map="$(python3 - "$driver_contract" <<'PY'
import re
import sys

text = open(sys.argv[1], encoding="utf-8").read()
pairs = re.findall(r'pub const ([A-Z0-9_]+): &str = "([A-Z0-9_]+)";', text)
if not pairs:
    raise SystemExit("the sing-box real-binary contract declares no environment keys")
for name, value in pairs:
    print(f"{name}={value}")
PY
)" || {
        printf '[FAIL] the sing-box real-binary contract could not be read\n' >&2
        return 1
    }
    while IFS='=' read -r const_name env_name; do
        case "$const_name" in
            BINARY_ENV) binary_env="$env_name" ;;
            CONFIG_ENV) config_env="$env_name" ;;
            INSTANCE_ENV) instance_env="$env_name" ;;
            CONTROLLER_ENV) controller_env="$env_name" ;;
            SECRET_ENV) secret_env="$env_name" ;;
            PROXY_ENV) proxy_env="$env_name" ;;
            FIXTURE_ENV) fixture_env="$env_name" ;;
            PROBE_OUTBOUND_ENV) probe_env="$env_name" ;;
            SELECTOR_ENV) selector_env="$env_name" ;;
            RECEIPT_ENV) receipt_env="$env_name" ;;
        esac
    done <<<"$contract_map"
    for required_key in "$binary_env" "$config_env" "$instance_env" "$controller_env" \
        "$proxy_env" "$fixture_env" "$probe_env" "$selector_env" "$receipt_env"; do
        if [[ -z "$required_key" ]]; then
            printf '[FAIL] the sing-box real-binary contract does not declare every environment key\n' >&2
            return 1
        fi
    done

    local driver_controller_port driver_socks_port driver_fixture_port driver_secret
    driver_controller_port="$(free_loopback_port)"
    driver_socks_port="$(free_loopback_port)"
    driver_fixture_port="$(free_loopback_port)"
    driver_secret="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')"
    local driver_profile="$driver_dir/config.json"
    python3 - "$driver_profile" "$driver_controller_port" "$driver_socks_port" \
        "$driver_fixture_port" "$driver_secret" <<'PY'
import json
import sys

path, controller, socks, fixture, secret = sys.argv[1:6]
document = {
    "log": {"level": "error"},
    "inbounds": [
        {
            "type": "socks",
            "tag": "socks-in",
            "listen": "127.0.0.1",
            "listen_port": int(socks),
        }
    ],
    "outbounds": [
        {
            "type": "selector",
            "tag": "SELECT",
            "outbounds": ["DIRECT", "BLOCK"],
            "default": "DIRECT",
        },
        {"type": "direct", "tag": "DIRECT"},
        {"type": "block", "tag": "BLOCK"},
    ],
    "experimental": {
        "clash_api": {
            "external_controller": f"127.0.0.1:{controller}",
            "secret": secret,
        },
        # The evidence run owns an isolated working directory, so the engine
        # must not persist its selection cache anywhere else.
        "cache_file": {"enabled": False},
    },
}
with open(path, "w", encoding="utf-8") as handle:
    json.dump(document, handle)
PY

    local receipt_file="$driver_dir/receipt.json"
    local test_log="$driver_dir/real-process-evidence.log"
    printf '[INFO] running the sing-box driver real-binary evidence against %s\n' "$expected_version"
    local test_status=0
    env "${binary_env}=$binary_path" \
        "${config_env}=$driver_profile" \
        "${instance_env}=real-process-evidence" \
        "${controller_env}=127.0.0.1:$driver_controller_port" \
        "${secret_env}=$driver_secret" \
        "${proxy_env}=127.0.0.1:$driver_socks_port" \
        "${fixture_env}=127.0.0.1:$driver_fixture_port" \
        "${probe_env}=DIRECT" \
        "${selector_env}=SELECT" \
        "${receipt_env}=$receipt_file" \
        cargo nextest run --offline -p singbox-driver \
            --test real_process_evidence --run-ignored only \
        >"$test_log" 2>&1 || test_status=$?

    local receipt_json=""
    if [[ -s "$receipt_file" ]]; then
        receipt_json="$(cat "$receipt_file")"
    fi
    if [[ -n "$receipt_json" ]]; then
        summary_receipts="${summary_receipts}${receipt_json}"$'\n'
    fi
    if [[ -z "$receipt_json" ]]; then
        printf '[FAIL] the sing-box driver evidence produced no receipt\n' >&2
        cat "$test_log" >&2 || true
        summary_add defects "driver_real_process_evidence" \
            "the sing-box driver evidence produced no receipt"
        return 1
    fi

    local receipt_summary
    receipt_summary="$(printf '%s' "$receipt_json" | python3 -c '
import json
import sys

payload = json.load(sys.stdin)
outcome = payload["outcome"]
proven = [item["proof"] for item in payload["proofs"] if item["outcome"]["kind"] == "proven"]
unsupported = [
    item["proof"] for item in payload["proofs"] if item["outcome"]["kind"] == "unsupported"
]
print("|".join([
    outcome["kind"],
    outcome.get("proof", ""),
    ",".join(proven),
    ",".join(unsupported),
]))
')" || {
        printf '[FAIL] the sing-box driver receipt could not be read\n' >&2
        summary_add defects "driver_real_process_evidence" \
            "the sing-box driver receipt could not be read"
        return 1
    }
    local receipt_kind receipt_proof receipt_proven receipt_unsupported
    IFS='|' read -r receipt_kind receipt_proof receipt_proven receipt_unsupported <<<"$receipt_summary"

    case "$receipt_kind" in
        complete)
            summary_add proven "driver_real_process_evidence" \
                "the sing-box driver proved ${receipt_proven} against ${expected_version}"
            printf '[PASS] sing-box driver real-binary evidence: %s\n' "$expected_version"
            ;;
        unsupported_proof)
            summary_add not_proven "driver_${receipt_proof}" \
                "the typed port reports the capability as unsupported"
            summary_add proven "driver_real_process_evidence" \
                "the sing-box driver proved ${receipt_proven} against ${expected_version}"
            summary_outcome="incomplete"
            printf '[PASS] sing-box driver real-binary evidence with one unsupported proof: %s\n' \
                "$expected_version"
            ;;
        environment_absent)
            summary_add skipped "driver_real_process_evidence" \
                "the environment does not provide a sing-box binary"
            summary_outcome="environment_absent"
            printf '[SKIP] the sing-box driver real-binary evidence is absent\n'
            ;;
        *)
            summary_add defects "driver_real_process_evidence" \
                "the sing-box driver evidence reported `${receipt_kind}`"
            cat "$test_log" >&2 || true
            return 1
            ;;
    esac
    if [[ "$test_status" -ne 0 && "$receipt_kind" != unsupported_proof ]]; then
        summary_add defects "driver_real_process_evidence" \
            "the sing-box driver evidence test failed"
        cat "$test_log" >&2 || true
        return 1
    fi
    return 0
}

if ! run_driver_evidence; then
    exit 1
fi

if [[ "$driver_only" != true && "$product_smokes" == true ]]; then
    SINGBOX_REAL_BINARY="$binary_path" \
    SINGBOX_REAL_VERSION="${expected_version#v}" \
        cargo nextest run --offline --locked -p infiltrator-composition \
            --test singbox_real_process_smoke --run-ignored only
fi

printf '[PASS] real sing-box evidence complete for %s\n' "$expected_version"
