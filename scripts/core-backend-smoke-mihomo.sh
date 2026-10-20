#!/usr/bin/env bash
# Real Mihomo evidence smoke: the locked release assets, the product-level
# typed smokes and the `mihomo-driver` real-binary proof.
#
# What this entry point produces
# ------------------------------
# * The locked Mihomo release asset and its extracted binary, both verified
#   against `packaging/mihomo-evidence-assets.json`.
# * The product-level typed smokes against the running engine.
# * The `mihomo-driver` real-binary proof: the named test target
#   `real_process_evidence` proves the controller handshake, the typed node
#   speed test, a real traffic flow through the engine's own listener and a
#   configuration write with its readback. Its environment contract lives in
#   `crates/mihomo-driver/tests/real_process_evidence/contract.rs`; this script
#   reads the environment key names from that file, so the script and the test
#   can never disagree about the contract.
# * One machine-readable summary line (`MUSIC_FROG_SMOKE_SUMMARY <json>`) that
#   names what was proven, which binary identity and digest were used, what
#   was skipped and why.
#
# Exit codes
# ----------
# 0  the summary reports `proven` or `environment_absent`: every step that
#    could run ran and passed, and the steps that could not run are listed in
#    `skipped` with their reason.
# 1  a real defect: an assertion failed, a digest mismatched, or the summary
#    could not be produced.
# 2  a usage error.
#
# Usage
# -----
#   scripts/core-backend-smoke-mihomo.sh [--self-test]
#                                         [--driver-evidence-only]
#                                         [--summary-file PATH]
#
# `MIHOMO_REAL_BINARY` may name an installed Mihomo executable. When it is set
# the script uses that binary instead of downloading the locked release asset,
# and the summary records that the release archive was not verified.
set -Eeuo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
evidence_lock="$repo_root/packaging/mihomo-evidence-assets.json"
evidence_guard="$repo_root/scripts/quality/mihomo-evidence-guard.py"
driver_contract="$repo_root/crates/mihomo-driver/tests/real_process_evidence/contract.rs"

driver_only=false
summary_file=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --self-test)
            python3 "$evidence_guard" --self-test
            python3 "$evidence_guard"
            exit 0
            ;;
        --driver-evidence-only)
            driver_only=true
            ;;
        --summary-file)
            shift
            summary_file="${1:-}"
            ;;
        *)
            printf '[FAIL] usage: %s [--self-test] [--driver-evidence-only] [--summary-file PATH]\n' \
                "${BASH_SOURCE[0]}" >&2
            exit 2
            ;;
    esac
    shift
done

for command_name in cargo curl env python3 sha256sum uname mktemp gzip awk seq sleep; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf '[FAIL] required command is unavailable: %s\n' "$command_name" >&2
        exit 1
    fi
done

if ! cargo nextest --version >/dev/null 2>&1; then
    printf '[FAIL] cargo-nextest is unavailable; real adapter evidence requires nextest\n' >&2
    exit 1
fi

if [[ "$(uname -s):$(uname -m)" != Linux:x86_64 && "$(uname -s):$(uname -m)" != Linux:amd64 ]]; then
    printf '[FAIL] evidence assets require Linux amd64; host is %s/%s\n' \
        "$(uname -s)" "$(uname -m)" >&2
    exit 1
fi

python3 "$evidence_guard"

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
driver_evidence_ran_for=""
env_binary_noted=false

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
    MUSIC_FROG_SMOKE_ENGINE="mihomo" \
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

process_id=""
probe_server_pid=""
connection_client_pid=""
run_dir=""

stop_process() {
    local label=$1
    local pid=$2
    if [[ -z "$pid" ]]; then
        return 0
    fi
    if kill -0 "$pid" 2>/dev/null; then
        kill -TERM "$pid" 2>/dev/null || true
        for _ in $(seq 1 50); do
            if ! kill -0 "$pid" 2>/dev/null; then
                break
            fi
            sleep 0.1
        done
        if kill -0 "$pid" 2>/dev/null; then
            kill -KILL "$pid" 2>/dev/null || true
        fi
    fi
    wait "$pid" 2>/dev/null || true
    if kill -0 "$pid" 2>/dev/null; then
        printf '[FAIL] cleanup could not confirm %s stopped (pid %s)\n' "$label" "$pid" >&2
        return 1
    fi
    return 0
}

cleanup_active_processes() {
    local stopped=true
    if ! stop_process 'active Mihomo connection fixture client' "$connection_client_pid"; then
        stopped=false
    fi
    if ! stop_process 'loopback probe server' "$probe_server_pid"; then
        stopped=false
    fi
    if ! stop_process 'Mihomo process' "$process_id"; then
        stopped=false
    fi
    if [[ "$stopped" == true ]]; then
        connection_client_pid=""
        probe_server_pid=""
        process_id=""
        return 0
    fi
    return 1
}

cleanup() {
    local exit_status=$?
    local had_active_process=false
    trap - EXIT INT TERM
    if [[ -n "$connection_client_pid" || -n "$probe_server_pid" || -n "$process_id" ]]; then
        had_active_process=true
    fi
    if ! cleanup_active_processes; then
        exit_status=1
    elif [[ "$had_active_process" == true && "$exit_status" -ne 0 ]]; then
        printf '[PASS] cleanup: active Mihomo and loopback probe processes stopped after failure\n'
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

run_driver_evidence() {
    # run_driver_evidence <expected_version> <binary_path> <binary_sha256>
    #
    # The driver proof runs against its own isolated engine: its own data
    # directory, its own configuration document, its own loopback controller
    # and its own listeners. The environment key names are read from the typed
    # contract of the driver, so the script and the test share one contract.
    local expected_version=$1
    local binary_path=$2
    local binary_digest=$3
    local driver_dir="$run_dir/driver-evidence"
    mkdir -m 700 -p "$driver_dir/home"

    local binary_env="" config_env="" home_env="" controller_env="" secret_env=""
    local proxy_env="" fixture_env="" probe_env="" selector_env="" receipt_env=""
    local contract_map
    contract_map="$(python3 - "$driver_contract" <<'PY'
import re
import sys

text = open(sys.argv[1], encoding="utf-8").read()
pairs = re.findall(r'pub const ([A-Z0-9_]+): &str = "([A-Z0-9_]+)";', text)
if not pairs:
    raise SystemExit("the Mihomo real-binary contract declares no environment keys")
for name, value in pairs:
    print(f"{name}={value}")
PY
)" || {
        printf '[FAIL] the Mihomo real-binary contract could not be read\n' >&2
        return 1
    }
    while IFS='=' read -r const_name env_name; do
        case "$const_name" in
            BINARY_ENV) binary_env="$env_name" ;;
            CONFIG_ENV) config_env="$env_name" ;;
            HOME_ENV) home_env="$env_name" ;;
            CONTROLLER_ENV) controller_env="$env_name" ;;
            SECRET_ENV) secret_env="$env_name" ;;
            PROXY_ENV) proxy_env="$env_name" ;;
            FIXTURE_ENV) fixture_env="$env_name" ;;
            PROBE_OUTBOUND_ENV) probe_env="$env_name" ;;
            SELECTOR_ENV) selector_env="$env_name" ;;
            RECEIPT_ENV) receipt_env="$env_name" ;;
        esac
    done <<<"$contract_map"
    for required_key in "$binary_env" "$config_env" "$home_env" "$controller_env" \
        "$proxy_env" "$fixture_env" "$probe_env" "$selector_env" "$receipt_env"; do
        if [[ -z "$required_key" ]]; then
            printf '[FAIL] the Mihomo real-binary contract does not declare every environment key\n' >&2
            return 1
        fi
    done

    local driver_controller_port driver_socks_port driver_fixture_port driver_secret
    driver_controller_port="$(free_loopback_port)"
    driver_socks_port="$(free_loopback_port)"
    driver_fixture_port="$(free_loopback_port)"
    driver_secret="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')"
    local driver_profile="$driver_dir/profile.yaml"
    cat >"$driver_profile" <<EOF
port: 0
socks-port: $driver_socks_port
mixed-port: 0
redir-port: 0
tproxy-port: 0
allow-lan: false
mode: rule
log-level: debug
ipv6: false
external-controller: 127.0.0.1:$driver_controller_port
secret: "$driver_secret"
proxies:
  - name: CORE-SMOKE-HTTP
    type: http
    server: 127.0.0.1
    port: $driver_fixture_port
proxy-groups:
  - name: CORE-SMOKE-SELECTOR
    type: select
    proxies:
      - DIRECT
      - CORE-SMOKE-HTTP
      - REJECT
rules:
  - IP-CIDR,127.0.0.0/8,DIRECT,no-resolve
  - MATCH,REJECT
EOF

    local receipt_file="$driver_dir/receipt.json"
    local test_log="$driver_dir/real-process-evidence.log"
    printf '[INFO] running the Mihomo driver real-binary evidence against %s\n' "$expected_version"
    local test_status=0
    env "${binary_env}=$binary_path" \
        "${config_env}=$driver_profile" \
        "${home_env}=$driver_dir/home" \
        "${controller_env}=http://127.0.0.1:$driver_controller_port" \
        "${secret_env}=$driver_secret" \
        "${proxy_env}=127.0.0.1:$driver_socks_port" \
        "${fixture_env}=127.0.0.1:$driver_fixture_port" \
        "${probe_env}=CORE-SMOKE-HTTP" \
        "${selector_env}=CORE-SMOKE-SELECTOR" \
        "${receipt_env}=$receipt_file" \
        cargo nextest run --offline -p mihomo-driver \
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
        printf '[FAIL] the Mihomo driver evidence produced no receipt\n' >&2
        cat "$test_log" >&2 || true
        summary_add defects "driver_real_process_evidence" \
            "the Mihomo driver evidence produced no receipt"
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
        printf '[FAIL] the Mihomo driver receipt could not be read\n' >&2
        summary_add defects "driver_real_process_evidence" \
            "the Mihomo driver receipt could not be read"
        return 1
    }
    local receipt_kind receipt_proof receipt_proven receipt_unsupported
    IFS='|' read -r receipt_kind receipt_proof receipt_proven receipt_unsupported <<<"$receipt_summary"

    case "$receipt_kind" in
        complete)
            summary_add proven "driver_real_process_evidence" \
                "the Mihomo driver proved ${receipt_proven} against ${expected_version}"
            printf '[PASS] Mihomo driver real-binary evidence: %s\n' "$expected_version"
            ;;
        unsupported_proof)
            summary_add not_proven "driver_${receipt_proof}" \
                "the typed port reports the capability as unsupported"
            summary_add proven "driver_real_process_evidence" \
                "the Mihomo driver proved ${receipt_proven} against ${expected_version}"
            summary_outcome="incomplete"
            printf '[PASS] Mihomo driver real-binary evidence with one unsupported proof: %s\n' \
                "$expected_version"
            ;;
        environment_absent)
            summary_add skipped "driver_real_process_evidence" \
                "the environment does not provide a Mihomo binary"
            summary_outcome="environment_absent"
            printf '[SKIP] the Mihomo driver real-binary evidence is absent\n'
            ;;
        *)
            summary_add defects "driver_real_process_evidence" \
                "the Mihomo driver evidence reported `${receipt_kind}`"
            cat "$test_log" >&2 || true
            return 1
            ;;
    esac
    if [[ "$test_status" -ne 0 && "$receipt_kind" != unsupported_proof ]]; then
        summary_add defects "driver_real_process_evidence" \
            "the Mihomo driver evidence test failed"
        cat "$test_log" >&2 || true
        return 1
    fi
    return 0
}

run_one_version() {
    temporary_root="${TMPDIR:-/tmp}"
    run_dir="$(mktemp -d "$temporary_root/music-frog-mihomo-smoke.XXXXXX")"
    chmod 700 "$run_dir"
    archive_path="$run_dir/$archive"
    binary_path="$run_dir/$binary_name"
    download_url="${release_url/\/tag\//\/download\/}/$archive"
    if [[ -n "${MIHOMO_REAL_BINARY:-}" ]]; then
        binary_path="$MIHOMO_REAL_BINARY"
        if [[ ! -x "$binary_path" ]]; then
            printf '[FAIL] MIHOMO_REAL_BINARY does not name an executable: %s\n' \
                "$binary_path" >&2
            exit 1
        fi
        actual_binary_digest="$(sha256sum "$binary_path" | awk '{print $1}')"
        if [[ "$env_binary_noted" != true ]]; then
            summary_add skipped "locked_release_asset" \
                "MIHOMO_REAL_BINARY was used; the locked release archive was not downloaded"
            env_binary_noted=true
        fi
        summary_binary_source="environment"
        if [[ "$actual_binary_digest" != "$binary_sha256" ]]; then
            printf '[INFO] the environment-provided binary does not match the locked %s digest; the product smokes of this leg are skipped\n' \
                "$expected_version"
            summary_add skipped "locked_product_smokes" \
                "the environment-provided binary does not match the locked ${expected_version} digest"
            product_smokes=false
        fi
    else
        printf '[INFO] downloading official asset: %s\n' "$download_url"
        if ! curl --fail --location --retry 2 --silent --show-error \
            --output "$archive_path" "$download_url"; then
            printf '[FAIL] official asset download failed for %s\n' "$expected_version" >&2
            exit 1
        fi
        actual_archive_digest="$(sha256sum "$archive_path" | awk '{print $1}')"
        if [[ "$actual_archive_digest" != "$archive_sha256" ]]; then
            printf '[FAIL] archive digest mismatch for %s\n' "$expected_version" >&2
            printf '       expected: %s\n       actual:   %s\n' \
                "$archive_sha256" "$actual_archive_digest" >&2
            exit 1
        fi
        if ! gzip -dc "$archive_path" >"$binary_path"; then
            printf '[FAIL] official release asset is not a valid gzip archive: %s\n' "$archive" >&2
            exit 1
        fi
        chmod 700 "$binary_path"
        actual_binary_digest="$(sha256sum "$binary_path" | awk '{print $1}')"
        if [[ "$actual_binary_digest" != "$binary_sha256" ]]; then
            printf '[FAIL] extracted binary digest mismatch for %s\n' "$expected_version" >&2
            printf '       expected: %s\n       actual:   %s\n' \
                "$binary_sha256" "$actual_binary_digest" >&2
            exit 1
        fi
        printf '[PASS] official asset lock: %s archive and binary SHA-256 verified\n' \
            "$expected_version"
        summary_binary_source="locked-release-asset"
    fi
    summary_binary_path="$binary_path"
    summary_binary_sha256="$actual_binary_digest"
    summary_binary_version="$("$binary_path" -v 2>/dev/null | awk 'NR==1 {print $3}' || true)"

    # The driver evidence runs once per distinct binary: the same executable
    # must not be proven twice under two different locked versions.
    if [[ "$driver_evidence_ran_for" != "$actual_binary_digest" ]]; then
        if ! run_driver_evidence "$expected_version" "$binary_path" "$actual_binary_digest"; then
            exit 1
        fi
        driver_evidence_ran_for="$actual_binary_digest"
    fi

    if [[ "$driver_only" == true || "$product_smokes" == false ]]; then
        cleanup_active_processes || true
        rm -rf -- "$run_dir"
        run_dir=""
        return 0
    fi

controller_port="$(python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
    listener.bind(("127.0.0.1", 0))
    print(listener.getsockname()[1])
PY
)"
socks_port="$(python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
    listener.bind(("127.0.0.1", 0))
    print(listener.getsockname()[1])
PY
)"
controller_secret="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')"
data_dir="$run_dir/data"
mkdir -m 700 "$data_dir"
mkdir -m 700 "$data_dir/providers"
configs_root="$data_dir"
profile_config_dir="$configs_root/engines/mihomo"
mkdir -m 700 -p "$profile_config_dir"
profile_path="$profile_config_dir/profile.yaml"
probe_server_port_file="$run_dir/probe-server-port"
dns_server_port_file="$run_dir/dns-server-port"
dns_alt_server_port_file="$run_dir/dns-alt-server-port"
active_proxy_ready_file="$run_dir/active-proxy-ready"
active_proxy_closed_file="$run_dir/active-proxy-closed"
rule_provider_update_signal_file="$run_dir/rule-provider-update-enabled"

python3 - "$probe_server_port_file" "$dns_server_port_file" "$dns_alt_server_port_file" "$active_proxy_ready_file" "$active_proxy_closed_file" "$rule_provider_update_signal_file" >"$run_dir/probe-server.log" 2>&1 <<'PY' &
import os
import socket
import socketserver
import struct
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

port_file = sys.argv[1]
dns_port_file = sys.argv[2]
dns_alt_port_file = sys.argv[3]
active_proxy_ready_file = sys.argv[4]
active_proxy_closed_file = sys.argv[5]
rule_provider_update_signal_file = sys.argv[6]

class ProbeDnsHandler(socketserver.BaseRequestHandler):
    def handle(self):
        packet, _ = self.request
        if len(packet) < 17:
            return
        offset = 12
        labels = []
        while offset < len(packet):
            size = packet[offset]
            offset += 1
            if size == 0:
                break
            if size & 0xC0 or offset + size > len(packet):
                return
            labels.append(packet[offset:offset + size].decode("ascii", "strict"))
            offset += size
        if offset + 4 > len(packet):
            return
        qtype, qclass = struct.unpack_from(">HH", packet, offset)
        question_end = offset + 4
        question = packet[12:question_end]
        if ".".join(labels).casefold() not in {
            "core-smoke.test",
            "core-smoke-active.test",
            "core-smoke-dns-updated.test",
            "core-smoke-dns-restored.test",
        } or qtype != 1 or qclass != 1:
            return
        request_flags = struct.unpack_from(">H", packet, 2)[0]
        response_flags = 0x8000 | (request_flags & 0x0100) | 0x0080
        header = packet[:2] + struct.pack(">HHHHH", response_flags, 1, 1, 0, 0)
        answer = (
            b"\xc0\x0c"
            + struct.pack(">HHIH", 1, 1, 60, 4)
            + socket.inet_aton(self.server.answer_ip)
        )
        self.request[1].sendto(header + question + answer, self.client_address)

class ProbeDnsServer(socketserver.ThreadingUDPServer):
    def __init__(self, answer_ip):
        self.answer_ip = answer_ip
        super().__init__(("127.0.0.1", 0), ProbeDnsHandler)

dns_server = ProbeDnsServer("192.0.2.42")
with open(dns_port_file, "w", encoding="utf-8") as destination:
    destination.write(str(dns_server.server_address[1]))
threading.Thread(target=dns_server.serve_forever, daemon=True).start()
dns_alt_server = ProbeDnsServer("192.0.2.53")
with open(dns_alt_port_file, "w", encoding="utf-8") as destination:
    destination.write(str(dns_alt_server.server_address[1]))
threading.Thread(target=dns_alt_server.serve_forever, daemon=True).start()

class ProbeProxyHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/rules":
            self.send_error(404)
            return
        payload = (
            b"alpha.test\nbeta.test\n"
            if os.path.exists(rule_provider_update_signal_file)
            else b"alpha.test\n"
        )
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(payload)
        self.wfile.flush()

    def do_CONNECT(self):
        self.send_response(200, "Connection Established")
        self.end_headers()
        self.wfile.flush()
        request_line = self.rfile.readline()
        if not request_line:
            return
        if request_line == b"GET /hold HTTP/1.1\r\n":
            while self.rfile.readline() not in (b"\r\n", b"\n", b""):
                pass
            with open(active_proxy_ready_file, "w", encoding="utf-8") as destination:
                destination.write("ready")
            while self.rfile.read(4096):
                pass
            with open(active_proxy_closed_file, "w", encoding="utf-8") as destination:
                destination.write("closed")
            return
        if request_line != b"HEAD /health HTTP/1.1\r\n":
            self.connection.sendall(
                b"HTTP/1.1 400 Bad Request\r\n"
                b"Content-Length: 0\r\n"
                b"Connection: close\r\n\r\n"
            )
            return
        while self.rfile.readline() not in (b"\r\n", b"\n", b""):
            pass
        time.sleep(0.01)
        self.wfile.write(
            b"HTTP/1.1 204 No Content\r\n"
            b"Content-Length: 0\r\n"
            b"Connection: close\r\n\r\n"
        )
        self.wfile.flush()

server = ThreadingHTTPServer(("127.0.0.1", 0), ProbeProxyHandler)
with open(port_file, "w", encoding="utf-8") as destination:
    destination.write(str(server.server_address[1]))
server.serve_forever()
PY
probe_server_pid=$!
for _ in $(seq 1 50); do
    if [[ -s "$probe_server_port_file" ]]; then
        break
    fi
    if ! kill -0 "$probe_server_pid" 2>/dev/null; then
        cat "$run_dir/probe-server.log" >&2
        printf '[FAIL] loopback probe server exited before binding\n' >&2
        exit 1
    fi
    sleep 0.1
done
if [[ ! -s "$probe_server_port_file" ]]; then
    cat "$run_dir/probe-server.log" >&2
    printf '[FAIL] loopback probe server did not report its port\n' >&2
    exit 1
fi
probe_server_port="$(cat "$probe_server_port_file")"
if [[ ! -s "$dns_server_port_file" ]]; then
    cat "$run_dir/probe-server.log" >&2
    printf '[FAIL] loopback DNS server did not report its port\n' >&2
    exit 1
fi
dns_server_port="$(cat "$dns_server_port_file")"
if [[ ! -s "$dns_alt_server_port_file" ]]; then
    cat "$run_dir/probe-server.log" >&2
    printf '[FAIL] alternate loopback DNS server did not report its port\n' >&2
    exit 1
fi
dns_alt_server_port="$(cat "$dns_alt_server_port_file")"
if [[ ! "$probe_server_port" =~ ^[0-9]+$ ]]; then
    printf '[FAIL] loopback probe server reported an invalid port\n' >&2
    exit 1
fi
if [[ ! "$dns_server_port" =~ ^[0-9]+$ ]]; then
    printf '[FAIL] loopback DNS server reported an invalid port\n' >&2
    exit 1
fi
if [[ ! "$dns_alt_server_port" =~ ^[0-9]+$ ]]; then
    printf '[FAIL] alternate loopback DNS server reported an invalid port\n' >&2
    exit 1
fi

cat >"$profile_path" <<EOF
port: 0
socks-port: $socks_port
redir-port: 0
tproxy-port: 0
mixed-port: 0
allow-lan: false
mode: rule
log-level: debug
ipv6: false
geo-auto-update: false
geodata-mode: true
geox-url:
  geoip: "http://127.0.0.1:$probe_server_port/geoip.dat"
  geosite: "http://127.0.0.1:$probe_server_port/geosite.dat"
  mmdb: "http://127.0.0.1:$probe_server_port/geoip.metadb"
  asn: "http://127.0.0.1:$probe_server_port/asn.mmdb"
external-controller: 127.0.0.1:$controller_port
secret: "$controller_secret"
dns:
  enable: true
  ipv6: false
  enhanced-mode: redir-host
  nameserver:
    - 127.0.0.1:$dns_server_port
tun:
  enable: false
  stack: system
proxies:
  - name: CORE-SMOKE-HTTP
    type: http
    server: 127.0.0.1
    port: $probe_server_port
proxy-groups:
  - name: CORE-SMOKE-SELECTOR
    type: select
    proxies:
      - DIRECT
      - CORE-SMOKE-HTTP
      - REJECT
rule-providers:
  CORE-SMOKE-RULE:
    type: http
    behavior: domain
    format: text
    url: http://127.0.0.1:$probe_server_port/rules
    path: ./providers/core-smoke-rules.txt
    interval: 3600
rules:
  - DOMAIN,core-smoke-active.test,CORE-SMOKE-HTTP
  - RULE-SET,CORE-SMOKE-RULE,DIRECT
  - IP-CIDR,127.0.0.0/8,DIRECT,no-resolve
  - MATCH,REJECT
EOF

if ! "$binary_path" -t -d "$data_dir" -f "$profile_path" \
    >"$run_dir/config-check.log" 2>&1; then
    cat "$run_dir/config-check.log" >&2
    printf '[FAIL] native config check rejected the isolated profile\n' >&2
    exit 1
fi
printf '[PASS] native config check: isolated profile accepted by %s\n' "$expected_version"

"$binary_path" -d "$data_dir" -f "$profile_path" \
    >"$run_dir/core.log" 2>&1 &
process_id=$!
base_url="http://127.0.0.1:$controller_port"
response_file="$run_dir/response.json"
api_status=""
api_body=""

api_request() {
    local method=$1
    local path=$2
    local request_body=${3-}
    local status
    local -a curl_args
    api_status="000"
    api_body="controller transport failed"

    curl_args=(
        --silent
        --show-error
        --noproxy '*'
        --connect-timeout 1
        --max-time 5
        --output "$response_file"
        --write-out '%{http_code}'
        --request "$method"
        --header "Authorization: Bearer $controller_secret"
    )
    if [[ -n "$request_body" ]]; then
        curl_args+=(
            --header 'Content-Type: application/json'
            --data "$request_body"
        )
    fi
    if ! status="$(curl "${curl_args[@]}" "$base_url$path")"; then
        return 1
    fi
    api_status=$status
    api_body="$(cat "$response_file")"
    [[ "$api_status" =~ ^2[0-9][0-9]$ ]]
}

probe_diagnostics() {
    printf '%s\n' '--- Mihomo log ---' >&2
    cat "$run_dir/core.log" >&2
    printf '%s\n' '--- loopback fixture log ---' >&2
    cat "$run_dir/probe-server.log" >&2
}

ready=false
for _ in $(seq 1 100); do
    if ! kill -0 "$process_id" 2>/dev/null; then
        cat "$run_dir/core.log" >&2
        printf '[FAIL] Mihomo exited before the controller became ready\n' >&2
        exit 1
    fi
    if api_request GET /version; then
        ready=true
        break
    fi
    sleep 0.1
done
if [[ "$ready" != true ]]; then
    cat "$run_dir/core.log" >&2
    printf '[FAIL] controller readiness timed out\n' >&2
    exit 1
fi

if ! python3 - "$response_file" "$expected_version" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
if payload.get("version") != sys.argv[2]:
    raise SystemExit(
        f"controller version {payload.get('version')!r} != locked {sys.argv[2]!r}"
    )
PY
then
    printf '[FAIL] live /version did not match the asset lock\n' >&2
    exit 1
fi
printf '[PASS] real controller /version: %s\n' "$expected_version"

if ! api_request GET '/dns/query?name=core-smoke.test&type=A'; then
    printf '[FAIL] DNS query returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    probe_diagnostics
    exit 1
fi
if ! python3 - "$response_file" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
if payload.get("Status") != 0:
    raise SystemExit(f"DNS RCODE is {payload.get('Status')!r}, expected 0")
if not any(
    answer.get("type") == 1 and answer.get("data") == "192.0.2.42"
    for answer in payload.get("Answer", [])
):
    raise SystemExit("DNS response has no expected A answer")
PY
then
    printf '[FAIL] live /dns/query response did not match the fixture answer\n' >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] live /dns/query endpoint: A core-smoke.test -> 192.0.2.42\n'

connection_client_ready_file="$run_dir/connection-client-ready"
connection_client_done_file="$run_dir/connection-client-done"
python3 - "$socks_port" "$connection_client_ready_file" "$connection_client_done_file" >"$run_dir/connection-client.log" 2>&1 <<'PY' &
import socket
import struct
import sys

proxy_port = int(sys.argv[1])
ready_file = sys.argv[2]
done_file = sys.argv[3]
host = b"core-smoke-active.test"
connection = socket.create_connection(("127.0.0.1", proxy_port), timeout=5)
try:
    connection.settimeout(5)
    connection.sendall(b"\x05\x01\x00")
    if connection.recv(2) != b"\x05\x00":
        raise RuntimeError("Mihomo SOCKS5 listener did not accept no-auth mode")
    connection.sendall(
        b"\x05\x01\x00\x03" + bytes([len(host)]) + host + struct.pack(">H", 80)
    )
    response = connection.recv(4)
    if len(response) != 4 or response[:2] != b"\x05\x00":
        raise RuntimeError(f"Mihomo SOCKS5 CONNECT failed: {response!r}")
    address_type = response[3]
    address_length = {1: 4, 4: 16}.get(address_type)
    if address_type == 3:
        address_length = connection.recv(1)[0]
    if address_length is None:
        raise RuntimeError(f"Mihomo SOCKS5 returned invalid address type {address_type}")
    remaining = address_length + 2
    while remaining:
        chunk = connection.recv(remaining)
        if not chunk:
            raise RuntimeError("Mihomo SOCKS5 reply ended early")
        remaining -= len(chunk)
    connection.sendall(
        b"GET /hold HTTP/1.1\r\n"
        b"Host: core-smoke-active.test\r\n"
        b"Connection: keep-alive\r\n\r\n"
    )
    with open(ready_file, "w", encoding="utf-8") as destination:
        destination.write("request sent")
    # The ignored adapter suite can trigger a first-time Rust build after the
    # fixture is opened. Keep the live connection available through that build
    # and the remaining sequential smoke checks; the EXIT trap still owns
    # deterministic cleanup when a check fails.
    connection.settimeout(300)
    while connection.recv(4096):
        pass
except ConnectionResetError:
    pass
finally:
    connection.close()
    with open(done_file, "w", encoding="utf-8") as destination:
        destination.write("done")
PY
connection_client_pid=$!
active_connection_ready=false
for _ in $(seq 1 50); do
    if [[ -s "$active_proxy_ready_file" ]]; then
        active_connection_ready=true
        break
    fi
    if ! kill -0 "$connection_client_pid" 2>/dev/null; then
        cat "$run_dir/connection-client.log" >&2
        printf '[FAIL] active connection fixture client exited before Mihomo observed it\n' >&2
        probe_diagnostics
        exit 1
    fi
    sleep 0.1
done
if [[ "$active_connection_ready" != true ]]; then
    printf '[FAIL] local HTTP proxy did not observe the active connection fixture\n' >&2
    cat "$run_dir/connection-client.log" >&2
    probe_diagnostics
    exit 1
fi
if ! api_request GET /connections; then
    printf '[FAIL] /connections could not read the active fixture connection (HTTP %s)\n' \
        "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    probe_diagnostics
    exit 1
fi
if ! python3 - "$response_file" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
connections = payload.get("connections")
if not isinstance(connections, list):
    raise SystemExit("/connections did not report the live list as an array")
if not any(
    item.get("metadata", {}).get("host") == "core-smoke-active.test"
    and isinstance(item.get("id"), str)
    and item["id"]
    for item in connections
):
    raise SystemExit("/connections did not report the active fixture connection")
PY
then
    printf '[FAIL] /connections did not report the active fixture connection\n' >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] live /connections reports the active core-smoke-active.test fixture\n'

if ! api_request GET /providers/rules; then
    printf '[FAIL] /providers/rules returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    probe_diagnostics
    exit 1
fi
provider_snapshot_body="$api_body"
if ! python3 - "$response_file" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
provider = payload.get("providers", {}).get("CORE-SMOKE-RULE")
if not isinstance(provider, dict) or provider.get("ruleCount") != 1:
    raise SystemExit("live rule-provider list has no one-rule fixture")
PY
then
    printf '[FAIL] /providers/rules did not return the initial fixture state\n' >&2
    printf '%s\n' "$provider_snapshot_body" >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] live /providers/rules reports CORE-SMOKE-RULE with ruleCount=1\n'

printf '[INFO] running ignored Rust adapter smoke against %s\n' "$expected_version"
if ! MIHOMO_REAL_CONTROLLER_URL="$base_url" \
    MIHOMO_REAL_CONTROLLER_SECRET="$controller_secret" \
    MIHOMO_REAL_VERSION="$expected_version" \
    MIHOMO_REAL_RULE_PROVIDER_UPDATE_SIGNAL="$rule_provider_update_signal_file" \
    cargo nextest run --offline --locked -p mihomo-api \
        --test real_process_smoke --run-ignored only; then
    printf '[FAIL] Rust typed adapter smoke failed for %s\n' "$expected_version" >&2
    printf '%s\n' '--- live /providers/rules before typed adapter read ---' "$provider_snapshot_body" >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] Rust typed DNS query, scoped Fake-IP flush, rule-provider list/update, Connection.Close, scoped GC with memory readback, log stream, log-level, outbound, and routing adapters drove %s\n' \
    "$expected_version"

printf '[INFO] running ignored Rust Geo update trigger smoke against %s\n' "$expected_version"
if ! MIHOMO_REAL_CONTROLLER_URL="$base_url" \
    MIHOMO_REAL_CONTROLLER_SECRET="$controller_secret" \
    MIHOMO_REAL_VERSION="$expected_version" \
    MIHOMO_REAL_PROFILE_PATH="$profile_path" \
    MIHOMO_REAL_GEO_FIXTURE_ORIGIN="http://127.0.0.1:$probe_server_port" \
    cargo nextest run --offline --locked -p mihomo-api \
        --test mihomo_geo_update_real_process_smoke --run-ignored only; then
    printf '[FAIL] Rust Geo update trigger smoke failed for %s\n' "$expected_version" >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] Geo update trigger was accepted with loopback geox URLs and a REJECT-default Mihomo profile for %s\n' \
    "$expected_version"

printf '[INFO] running ignored Rust TUN configuration application smoke against %s\n' "$expected_version"
if ! MIHOMO_REAL_CONTROLLER_URL="$base_url" \
    MIHOMO_REAL_CONTROLLER_SECRET="$controller_secret" \
    MIHOMO_REAL_VERSION="$expected_version" \
    cargo nextest run --offline --locked -p infiltrator-composition \
        --test mihomo_tun_configuration_real_process_smoke --run-ignored only; then
    printf '[FAIL] Rust TUN configuration smoke failed for %s\n' "$expected_version" >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] TUN stack typed apply/readback/restore kept the TUN disabled for %s\n' \
    "$expected_version"

printf '[INFO] running ignored Rust Profile.ApplyFeature smoke against %s\n' "$expected_version"
if ! MIHOMO_REAL_CONTROLLER_URL="$base_url" \
    MIHOMO_REAL_CONTROLLER_SECRET="$controller_secret" \
    MIHOMO_REAL_VERSION="$expected_version" \
    cargo nextest run --offline --locked -p infiltrator-composition \
        --test mihomo_profile_feature_real_process_smoke --run-ignored only; then
    printf '[FAIL] Rust Profile.ApplyFeature smoke failed for %s\n' "$expected_version" >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] Profile.ApplyFeature IPv6 write/readback/restore kept the host TUN disabled for %s\n' \
    "$expected_version"

printf '[INFO] running ignored Rust Rule.Trace application smoke against %s\n' "$expected_version"
if ! env -u INFILTRATOR_CONFIGS_DIR \
    MIHOMO_REAL_CONTROLLER_URL="$base_url" \
    MIHOMO_REAL_CONTROLLER_SECRET="$controller_secret" \
    MIHOMO_REAL_VERSION="$expected_version" \
    MIHOMO_REAL_CONFIGS_ROOT="$configs_root" \
    MIHOMO_REAL_PROFILE_PATH="$profile_path" \
    cargo nextest run --offline --locked -p infiltrator-composition \
        --test mihomo_rule_trace_real_process_smoke --run-ignored only; then
    printf '[FAIL] Rust Rule.Trace smoke failed for %s\n' "$expected_version" >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] read-only Rule.Trace resolved the live profile rule against Mihomo outbound evidence for %s\n' \
    "$expected_version"

for _ in $(seq 1 50); do
    if [[ -s "$connection_client_done_file" ]]; then
        break
    fi
    sleep 0.1
done
if [[ ! -s "$connection_client_done_file" ]]; then
    printf '[FAIL] active fixture client remained open after typed Connection.Close\n' >&2
    probe_diagnostics
    exit 1
fi
wait "$connection_client_pid" || {
    printf '[FAIL] active fixture client failed while observing Connection.Close\n' >&2
    cat "$run_dir/connection-client.log" >&2
    exit 1
}
connection_client_pid=""
if [[ ! -s "$active_proxy_closed_file" ]]; then
    printf '[FAIL] local HTTP proxy did not observe Mihomo closing the active connection\n' >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] typed Connection.Close ended the live SOCKS→Mihomo→HTTP fixture\n'

printf '[INFO] running ignored Rust Profile transaction smoke against %s\n' "$expected_version"
if ! env -u INFILTRATOR_CONFIGS_DIR \
    MIHOMO_REAL_CONTROLLER_URL="$base_url" \
    MIHOMO_REAL_CONTROLLER_SECRET="$controller_secret" \
    MIHOMO_REAL_VERSION="$expected_version" \
    MIHOMO_REAL_BINARY="$binary_path" \
    MIHOMO_REAL_CONFIGS_ROOT="$configs_root" \
    MIHOMO_REAL_PROFILE_PATH="$profile_path" \
    MIHOMO_REAL_DNS_ALT_SERVER="127.0.0.1:$dns_alt_server_port" \
    cargo nextest run --offline --locked -p infiltrator-core \
        --test real_process_profile_smoke --run-ignored only; then
    printf '[FAIL] Rust Profile transaction smoke failed for %s\n' "$expected_version" >&2
    python3 - "$run_dir/core.log" <<'PY' >&2
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    diagnostics = [
        line.rstrip()
        for line in source
        if "level=error" in line.casefold() or "level=warning" in line.casefold()
    ]
for line in diagnostics:
    if "secret" in line.casefold() or "token" in line.casefold():
        print("[REDACTED] Mihomo diagnostic contained secret material")
    else:
        print(line)
PY
    exit 1
fi
printf '[PASS] Rust Profile, DNS and Rules transaction apply/readback/restore drove %s\n' \
    "$expected_version"

printf '[INFO] running ignored Rust subscription application smoke against %s\n' "$expected_version"
if ! env -u INFILTRATOR_CONFIGS_DIR \
    MIHOMO_REAL_VERSION="$expected_version" \
    MIHOMO_REAL_BINARY="$binary_path" \
    MIHOMO_REAL_CONFIGS_ROOT="$configs_root" \
    cargo nextest run --offline --locked -p infiltrator-core \
        --test mihomo_subscription_real_process_smoke --run-ignored only; then
    printf '[FAIL] Rust subscription application smoke failed for %s\n' "$expected_version" >&2
    exit 1
fi
printf '[PASS] loopback subscription Import/UpdateProfile/UpdateAllSubscriptions and native profile checks passed for %s\n' \
    "$expected_version"

reload_body="$(python3 - "$profile_path" <<'PY'
import json
import sys

print(json.dumps({"path": sys.argv[1]}))
PY
)"
if ! api_request PUT '/configs?force=true' "$reload_body"; then
    printf '[FAIL] profile reload returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    exit 1
fi
if ! api_request GET /configs; then
    printf '[FAIL] profile readback returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    exit 1
fi
if ! python3 - "$response_file" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
if payload.get("mode") != "rule" or payload.get("log-level") != "debug":
    raise SystemExit("mode/log-level readback did not match the applied profile")
PY
then
    printf '[FAIL] profile readback did not match the applied profile\n' >&2
    exit 1
fi
printf '[PASS] profile reload and /configs readback: mode=rule log-level=debug\n'

if ! api_request GET /proxies; then
    printf '[FAIL] /proxies returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    exit 1
fi
if ! python3 - "$response_file" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
proxy = payload.get("proxies", {}).get("CORE-SMOKE-SELECTOR")
if not isinstance(proxy, dict) or proxy.get("type") != "Selector":
    raise SystemExit("configured selector is absent from /proxies")
if proxy.get("now") != "DIRECT":
    raise SystemExit(f"selector initial value is {proxy.get('now')!r}, expected DIRECT")
PY
then
    printf '[FAIL] /proxies did not report the configured selector\n' >&2
    exit 1
fi
printf '[PASS] /proxies list: configured selector and initial selection observed\n'

if ! api_request PUT '/proxies/CORE-SMOKE-SELECTOR' '{"name":"REJECT"}'; then
    printf '[FAIL] selector write returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    exit 1
fi
if ! api_request GET '/proxies/CORE-SMOKE-SELECTOR'; then
    printf '[FAIL] selector readback returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    exit 1
fi
if ! python3 - "$response_file" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
if payload.get("now") != "REJECT":
    raise SystemExit(f"selector readback is {payload.get('now')!r}, expected REJECT")
PY
then
    printf '[FAIL] selector readback did not report REJECT\n' >&2
    exit 1
fi
printf '[PASS] selector write/readback: now=REJECT\n'

probe_url="http://probe.test/health"
encoded_probe_url="$(python3 - "$probe_url" <<'PY'
import sys
from urllib.parse import quote

print(quote(sys.argv[1], safe=""))
PY
)"
if ! api_request GET "/proxies/CORE-SMOKE-HTTP/delay?url=$encoded_probe_url&timeout=5000&expected=204"; then
    printf '[FAIL] HTTP outbound probe returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    probe_diagnostics
    exit 1
fi
probe_delay_ms="$(python3 - "$response_file" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
delay = payload.get("delay")
if not isinstance(delay, int) or delay < 0:
    raise SystemExit("outbound probe response has no nonnegative integer delay")
print(delay)
PY
)" || {
    printf '[FAIL] HTTP outbound probe returned an invalid response\n' >&2
    probe_diagnostics
    exit 1
}
printf '[PASS] outbound probe: CORE-SMOKE-HTTP returned %s ms through a local HTTP proxy\n' \
    "$probe_delay_ms"

if ! api_request PATCH /configs '{"log-level":"error"}'; then
    printf '[FAIL] log-level write returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    exit 1
fi
if ! api_request GET /configs; then
    printf '[FAIL] log-level readback returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    exit 1
fi
if ! python3 - "$response_file" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
if payload.get("log-level") != "error":
    raise SystemExit(f"log-level readback is {payload.get('log-level')!r}, expected error")
PY
then
    printf '[FAIL] log-level readback did not report error\n' >&2
    exit 1
fi
printf '[PASS] log-level write/readback: error (debug endpoint remains enabled from process startup)\n'

for mode in global direct rule; do
    if ! api_request PATCH /configs "{\"mode\":\"$mode\"}"; then
        printf '[FAIL] routing mode write returned HTTP %s\n' "$api_status" >&2
        printf '%s\n' "$api_body" >&2
        exit 1
    fi
    if ! api_request GET /configs; then
        printf '[FAIL] routing mode readback returned HTTP %s\n' "$api_status" >&2
        printf '%s\n' "$api_body" >&2
        exit 1
    fi
    if ! python3 - "$response_file" "$mode" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
if payload.get("mode") != sys.argv[2]:
    raise SystemExit(
        f"routing mode readback is {payload.get('mode')!r}, expected {sys.argv[2]!r}"
    )
PY
    then
        printf '[FAIL] routing mode readback did not match %s\n' "$mode" >&2
        exit 1
    fi
    printf '[PASS] routing intent write/readback: %s\n' "$mode"
done

if ! api_request GET /connections; then
    printf '[FAIL] /connections returned HTTP %s\n' "$api_status" >&2
    printf '%s\n' "$api_body" >&2
    exit 1
fi
connection_summary="$(python3 - "$response_file" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    payload = json.load(source)
connections = payload.get("connections", "missing")
if connections != "missing" and connections is not None and not isinstance(connections, list):
    raise SystemExit("/connections value is neither null nor an array")
if connections == "missing":
    raise SystemExit("/connections response has no connections field")
if not isinstance(payload.get("downloadTotal"), int):
    raise SystemExit("/connections response has no integer downloadTotal")
if not isinstance(payload.get("uploadTotal"), int):
    raise SystemExit("/connections response has no integer uploadTotal")
if connections is None:
    print("connections=null")
else:
    print(f"connections=array({len(connections)})")
PY
)" || {
    printf '[FAIL] /connections response shape did not match the observed contract\n' >&2
    exit 1
}
printf '[PASS] /connections read: %s; counters observed\n' "$connection_summary"

printf '[NOT COVERED] Logs.Read/Clear, host TUN device/VPN activation, DNS cache contents, Fake-IP cache readback, proxy providers/healthcheck, Connection.List, non-Linux hosts\n'

if ! cleanup_active_processes; then
    exit 1
fi
rm -rf -- "$run_dir"
run_dir=""
printf '[PASS] cleanup: isolated Mihomo and loopback probe processes stopped for %s\n' \
    "$expected_version"
}

while IFS=$'\t' read -r expected_version release_url archive archive_sha256 binary_name binary_sha256; do
    run_one_version
done < <(python3 - "$evidence_lock" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    lock = json.load(source)
for asset in lock["assets"]:
    print("\t".join((
        asset["version"],
        asset["release_url"],
        asset["archive"],
        asset["archive_sha256"],
        asset["binary"],
        asset["binary_sha256"],
    )))
PY
)

printf '[PASS] real Mihomo evidence complete for all locked releases\n'
