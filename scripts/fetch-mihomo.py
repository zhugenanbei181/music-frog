#!/usr/bin/env python3
"""Restore pinned kernel assets. Both archive and extracted bytes must match the lock."""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
LOCK = ROOT / "packaging/mihomo-assets.json"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def validate_lock(lock):
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", lock["version"]):
        raise ValueError("invalid pinned version")
    targets = set()
    outputs = set()
    for asset in lock["assets"]:
        if asset["target"] in targets or asset["binary"] in outputs:
            raise ValueError("duplicate target or output in kernel lock")
        targets.add(asset["target"])
        outputs.add(asset["binary"])
        for name in (asset["archive"], asset["binary"]):
            if not re.fullmatch(r"[a-zA-Z0-9_.-]+", name):
                raise ValueError("unsafe asset filename")
        for key in ("archive_sha256", "binary_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", asset[key]):
                raise ValueError("every kernel asset requires a SHA-256 digest")
    if not targets:
        raise ValueError("empty kernel lock")


def decode_archive(name, data):
    if name.endswith(".gz"):
        return gzip.decompress(data)
    if name.endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            executables = [info for info in archive.infolist() if not info.is_dir() and info.filename.endswith(".exe")]
            if len(executables) != 1:
                raise ValueError("kernel zip must contain exactly one executable")
            return archive.read(executables[0])
    raise ValueError("unsupported kernel archive format")


def fetch_asset(asset, version, base, vendor):
    output = vendor / asset["binary"]
    if output.is_file() and sha256(output.read_bytes()) == asset["binary_sha256"]:
        output.chmod(0o755)
        print(f"[fetch-mihomo] verified cached {output.name}", flush=True)
        return
    url = f"{base}/{version}/{asset['archive']}"
    with urllib.request.urlopen(url, timeout=60) as response:
        archive = response.read()
    if sha256(archive) != asset["archive_sha256"]:
        raise ValueError(f"archive checksum mismatch: {asset['archive']}")
    binary = decode_archive(asset["archive"], archive)
    if sha256(binary) != asset["binary_sha256"]:
        raise ValueError(f"extracted kernel checksum mismatch: {asset['binary']}")
    write_atomic(output, binary)
    print(f"[fetch-mihomo] verified and installed {output.name}", flush=True)


def write_atomic(output, binary, mode=0o755):
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=output.parent, prefix=".kernel-", delete=False) as file:
            temporary = Path(file.name)
            file.write(binary)
        temporary.chmod(mode)
        os.replace(temporary, output)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", action="append", help="restore only this Rust target (repeatable)")
    parser.add_argument("--stage", type=Path, help="copy the verified kernel for one target into this package path")
    args = parser.parse_args()
    lock = json.loads(LOCK.read_text())
    validate_lock(lock)
    requested = os.environ.get("MIHOMO_VERSION", lock["version"])
    if requested != lock["version"]:
        raise ValueError("unlocked MIHOMO_VERSION refused; update packaging/mihomo-assets.json with verified digests")
    base = os.environ.get("MIHOMO_BASE_URL", "https://github.com/MetaCubeX/mihomo/releases/download").rstrip("/")
    if not re.fullmatch(r"https?://[A-Za-z0-9._~:/%-]+", base):
        raise ValueError("invalid kernel download base URL")
    assets = lock["assets"]
    if args.target:
        targets = set(args.target)
        available = {asset["target"] for asset in assets}
        if not targets <= available:
            raise ValueError(f"unlocked targets refused: {sorted(targets - available)}")
        assets = [asset for asset in assets if asset["target"] in targets]
    if args.stage is not None and len(assets) != 1:
        raise ValueError("package staging requires exactly one locked target")
    vendor = ROOT / "vendor"
    vendor.mkdir(exist_ok=True)
    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(lambda asset: fetch_asset(asset, lock["version"], base, vendor), assets))
    if args.stage is not None:
        asset = assets[0]
        binary = (vendor / asset["binary"]).read_bytes()
        if sha256(binary) != asset["binary_sha256"]:
            raise ValueError("cached kernel changed before package staging")
        write_atomic(args.stage, binary)
        write_atomic(args.stage.parent / "Mihomo-LICENSE.txt", (ROOT / "packaging/Mihomo-LICENSE").read_bytes(), 0o644)
        write_atomic(args.stage.parent / "mihomo-assets.json", LOCK.read_bytes(), 0o644)
    print(f"[fetch-mihomo] {lock['version']}: {len(assets)} verified assets")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, json.JSONDecodeError) as error:
        raise SystemExit(f"[fetch-mihomo] ERROR: {error}") from error
