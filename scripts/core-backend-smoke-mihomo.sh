#!/usr/bin/env bash
set -Eeuo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
evidence_lock="$repo_root/packaging/mihomo-evidence-assets.json"
evidence_guard="$repo_root/scripts/quality/mihomo-evidence-guard.py"

if [[ $# -eq 1 && "$1" == "--self-test" ]]; then
    python3 "$evidence_guard" --self-test
    python3 "$evidence_guard"
    exit 0
fi
if [[ $# -ne 0 ]]; then
    printf '[FAIL] usage: %s [--self-test]\n' "${BASH_SOURCE[0]}" >&2
    exit 2
fi

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

process_id=""
probe_server_pid=""
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
    if ! stop_process 'loopback probe server' "$probe_server_pid"; then
        stopped=false
    fi
    if ! stop_process 'Mihomo process' "$process_id"; then
        stopped=false
    fi
    if [[ "$stopped" == true ]]; then
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
    if [[ -n "$probe_server_pid" || -n "$process_id" ]]; then
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
    exit "$exit_status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

run_one_version() {
    temporary_root="${TMPDIR:-/tmp}"
    run_dir="$(mktemp -d "$temporary_root/music-frog-mihomo-smoke.XXXXXX")"
    chmod 700 "$run_dir"
    archive_path="$run_dir/$archive"
    binary_path="$run_dir/$binary_name"
    download_url="${release_url/\/tag\//\/download\/}/$archive"
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

controller_port="$(python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
    listener.bind(("127.0.0.1", 0))
    print(listener.getsockname()[1])
PY
)"
controller_secret="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')"
data_dir="$run_dir/data"
mkdir -m 700 "$data_dir"
configs_root="$data_dir"
profile_config_dir="$configs_root/engines/mihomo"
mkdir -m 700 -p "$profile_config_dir"
profile_path="$profile_config_dir/profile.yaml"
probe_server_port_file="$run_dir/probe-server-port"
dns_server_port_file="$run_dir/dns-server-port"

python3 - "$probe_server_port_file" "$dns_server_port_file" >"$run_dir/probe-server.log" 2>&1 <<'PY' &
import socket
import socketserver
import struct
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

port_file = sys.argv[1]
dns_port_file = sys.argv[2]

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
        if ".".join(labels).casefold() != "core-smoke.test" or qtype != 1 or qclass != 1:
            return
        request_flags = struct.unpack_from(">H", packet, 2)[0]
        response_flags = 0x8000 | (request_flags & 0x0100) | 0x0080
        header = packet[:2] + struct.pack(">HHHHH", response_flags, 1, 1, 0, 0)
        answer = b"\xc0\x0c" + struct.pack(">HHIH", 1, 1, 60, 4) + socket.inet_aton("192.0.2.42")
        self.request[1].sendto(header + question + answer, self.client_address)

dns_server = socketserver.ThreadingUDPServer(("127.0.0.1", 0), ProbeDnsHandler)
with open(dns_port_file, "w", encoding="utf-8") as destination:
    destination.write(str(dns_server.server_address[1]))
threading.Thread(target=dns_server.serve_forever, daemon=True).start()

class ProbeProxyHandler(BaseHTTPRequestHandler):
    def do_CONNECT(self):
        self.send_response(200, "Connection Established")
        self.end_headers()
        self.wfile.flush()
        request_line = self.rfile.readline()
        if not request_line:
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
if [[ ! "$probe_server_port" =~ ^[0-9]+$ ]]; then
    printf '[FAIL] loopback probe server reported an invalid port\n' >&2
    exit 1
fi
if [[ ! "$dns_server_port" =~ ^[0-9]+$ ]]; then
    printf '[FAIL] loopback DNS server reported an invalid port\n' >&2
    exit 1
fi

cat >"$profile_path" <<EOF
port: 0
socks-port: 0
redir-port: 0
tproxy-port: 0
mixed-port: 0
allow-lan: false
mode: rule
log-level: info
ipv6: false
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
rules:
  - MATCH,DIRECT
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

printf '[INFO] running ignored Rust adapter smoke against %s\n' "$expected_version"
if ! MIHOMO_REAL_CONTROLLER_URL="$base_url" \
    MIHOMO_REAL_CONTROLLER_SECRET="$controller_secret" \
    MIHOMO_REAL_VERSION="$expected_version" \
    cargo nextest run --offline --locked -p mihomo-api \
        --test real_process_smoke --run-ignored only; then
    printf '[FAIL] Rust typed adapter smoke failed for %s\n' "$expected_version" >&2
    probe_diagnostics
    exit 1
fi
printf '[PASS] Rust typed DNS query, log stream, log-level, outbound, and routing adapters drove %s\n' \
    "$expected_version"

printf '[INFO] running ignored Rust Profile transaction smoke against %s\n' "$expected_version"
if ! env -u INFILTRATOR_CONFIGS_DIR \
    MIHOMO_REAL_CONTROLLER_URL="$base_url" \
    MIHOMO_REAL_CONTROLLER_SECRET="$controller_secret" \
    MIHOMO_REAL_VERSION="$expected_version" \
    MIHOMO_REAL_CONFIGS_ROOT="$configs_root" \
    MIHOMO_REAL_PROFILE_PATH="$profile_path" \
    cargo nextest run --offline --locked -p infiltrator-core \
        --test real_process_profile_smoke --run-ignored only; then
    printf '[FAIL] Rust Profile transaction smoke failed for %s\n' "$expected_version" >&2
    exit 1
fi
printf '[PASS] Rust Profile transaction apply/readback/restore drove %s\n' \
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
if payload.get("mode") != "rule" or payload.get("log-level") != "info":
    raise SystemExit("mode/log-level readback did not match the applied profile")
PY
then
    printf '[FAIL] profile readback did not match the applied profile\n' >&2
    exit 1
fi
printf '[PASS] profile reload and /configs readback: mode=rule log-level=info\n'

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

if ! api_request PATCH /configs '{"log-level":"debug"}'; then
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
if payload.get("log-level") != "debug":
    raise SystemExit(f"log-level readback is {payload.get('log-level')!r}, expected debug")
PY
then
    printf '[FAIL] log-level readback did not report debug\n' >&2
    exit 1
fi
printf '[PASS] log-level write/readback: debug\n'

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

printf '[NOT COVERED] Logs.Read/Clear, TUN/VPN, DNS cache, rules trace/apply, providers, connection close, non-Linux hosts\n'

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
