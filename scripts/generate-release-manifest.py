#!/usr/bin/env python3
"""
Release Manifest and Integrity Catalog Generator for Infiltrator.
Scans release artifacts in dist/, computes SHA-256 digests, infers platform targets,
and outputs standardized `release-manifest.json` and `latest.json` for client self-updaters.
"""

import argparse
import hashlib
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path


def compute_sha256(file_path: Path) -> str:
    hasher = hashlib.sha256()
    with open(file_path, "rb") as f:
        while chunk := f.read(65536):
            hasher.update(chunk)
    return hasher.hexdigest()


def infer_target_info(filename: str) -> dict:
    name_lower = filename.lower()
    info = {
        "pkg_type": "unknown",
        "os": "unknown",
        "arch": "unknown",
        "target_triple": "unknown",
    }

    if name_lower.endswith(".msi"):
        info["pkg_type"] = "windows_msi"
        info["os"] = "windows"
        info["arch"] = "x86_64"
        info["target_triple"] = "x86_64-pc-windows-msvc"
    elif name_lower.endswith(".exe"):
        info["pkg_type"] = "windows_nsis"
        info["os"] = "windows"
        if "arm64" in name_lower or "aarch64" in name_lower:
            info["arch"] = "aarch64"
            info["target_triple"] = "aarch64-pc-windows-msvc"
        else:
            info["arch"] = "x86_64"
            info["target_triple"] = "x86_64-pc-windows-msvc"
    elif name_lower.endswith(".dmg"):
        info["pkg_type"] = "macos_dmg"
        info["os"] = "macos"
        if "arm64" in name_lower or "aarch64" in name_lower:
            info["arch"] = "aarch64"
            info["target_triple"] = "aarch64-apple-darwin"
        elif "x86_64" in name_lower or "x64" in name_lower:
            info["arch"] = "x86_64"
            info["target_triple"] = "x86_64-apple-darwin"
        else:
            info["arch"] = "universal"
            info["target_triple"] = "universal2-apple-darwin"
    elif name_lower.endswith(".appimage"):
        info["pkg_type"] = "linux_appimage"
        info["os"] = "linux"
        info["arch"] = "x86_64"
        info["target_triple"] = "x86_64-unknown-linux-gnu"
    elif name_lower.endswith(".deb"):
        info["pkg_type"] = "linux_deb"
        info["os"] = "linux"
        if "arm64" in name_lower:
            info["arch"] = "aarch64"
            info["target_triple"] = "aarch64-unknown-linux-gnu"
        else:
            info["arch"] = "x86_64"
            info["target_triple"] = "x86_64-unknown-linux-gnu"
    elif name_lower.endswith(".apk"):
        info["pkg_type"] = "android_apk"
        info["os"] = "android"
        if "arm64" in name_lower or "v8a" in name_lower:
            info["arch"] = "aarch64"
            info["target_triple"] = "aarch64-linux-android"
        elif "x86_64" in name_lower:
            info["arch"] = "x86_64"
            info["target_triple"] = "x86_64-linux-android"
        elif "armv7" in name_lower or "v7a" in name_lower:
            info["arch"] = "armv7"
            info["target_triple"] = "armv7-linux-androideabi"
        else:
            info["arch"] = "universal"
            info["target_triple"] = "universal-android"
    elif name_lower.endswith(".zip") or name_lower.endswith(".tar.gz"):
        info["pkg_type"] = "archive"
        if "windows" in name_lower:
            info["os"] = "windows"
            info["arch"] = "aarch64" if "aarch64" in name_lower or "arm64" in name_lower else "x86_64"
            info["target_triple"] = f"{info['arch']}-pc-windows-msvc"
        elif "macos" in name_lower or "darwin" in name_lower:
            info["os"] = "macos"
            info["arch"] = "aarch64" if "arm64" in name_lower else "x86_64"
            info["target_triple"] = f"{info['arch']}-apple-darwin"
        elif "linux" in name_lower:
            info["os"] = "linux"
            info["arch"] = "aarch64" if "aarch64" in name_lower or "arm64" in name_lower else "x86_64"
            info["target_triple"] = f"{info['arch']}-unknown-linux-gnu"

    return info


def main():
    parser = argparse.ArgumentParser(description="Generate Infiltrator Release Manifests")
    parser.add_argument("--version", required=True, help="Release version (e.g., 0.20.0)")
    parser.add_argument(
        "--repository",
        default=os.environ.get("GITHUB_REPOSITORY", "zhugenanbei181/music-frog"),
        help="GitHub owner/repository used for artifact download URLs",
    )
    parser.add_argument("--channel", default="stable", choices=["stable", "beta", "nightly"], help="Release channel")
    parser.add_argument("--dist-dir", default="dist", help="Directory containing release build artifacts")
    parser.add_argument("--output-manifest", default="release-manifest.json", help="Path for full manifest output")
    parser.add_argument("--output-latest", default="latest.json", help="Path for client updater payload output")
    parser.add_argument("--output-sha256sums", default="SHA256SUMS.txt", help="Path for SHA256SUMS text file output")
    parser.add_argument("--write-individual-sha256", action="store_true", help="Write individual .sha256 files for each artifact")
    parser.add_argument("--min-version", default="0.19.0", help="Minimum supported version for upgrading")
    parser.add_argument("--critical", action="store_true", help="Flag release as critical security fix")
    parser.add_argument("--rollout", type=int, default=100, help="Phased rollout percentage (0-100)")
    args = parser.parse_args()

    dist_path = Path(args.dist_dir)
    if not dist_path.exists():
        print(f"[generate-release-manifest] Warning: Dist directory {dist_path} does not exist. Creating empty manifest.")
        artifacts = []
    else:
        artifacts = []
        for file in sorted(dist_path.iterdir()):
            if file.is_file() and not file.name.endswith(".json") and not file.name.endswith(".txt") and not file.name.endswith(".sha256"):
                sha256_hex = compute_sha256(file)
                size_bytes = file.stat().st_size
                target_info = infer_target_info(file.name)
                artifacts.append({
                    "name": file.name,
                    "target_triple": target_info["target_triple"],
                    "pkg_type": target_info["pkg_type"],
                    "os": target_info["os"],
                    "arch": target_info["arch"],
                    "sha256": sha256_hex,
                    "size_bytes": size_bytes,
                    "download_url": f"https://github.com/{args.repository}/releases/download/v{args.version}/{file.name}",
                    "signature": None,
                })
                if args.write_individual_sha256:
                    sha_file = file.with_name(f"{file.name}.sha256")
                    with open(sha_file, "w", encoding="utf-8") as sf:
                        sf.write(f"{sha256_hex}  {file.name}\n")

    now_iso = datetime.now(timezone.utc).isoformat()

    full_manifest = {
        "version": args.version,
        "channel": args.channel,
        "release_date": now_iso,
        "release_notes": f"Release v{args.version}",
        "min_supported_version": args.min_version,
        "critical_security_fix": args.critical,
        "rollout_percentage": args.rollout,
        "artifacts": artifacts,
        "deltas": [],
    }

    # Write release-manifest.json
    with open(args.output_manifest, "w", encoding="utf-8") as f:
        json.dump(full_manifest, f, indent=2, ensure_ascii=False)
    print(f"[generate-release-manifest] Wrote full manifest with {len(artifacts)} artifacts to {args.output_manifest}")

    # Write latest.json (updater feed)
    latest_payload = {
        "version": args.version,
        "channel": args.channel,
        "published_at": now_iso,
        "min_supported_version": args.min_version,
        "critical": args.critical,
        "rollout_percentage": args.rollout,
        # Legacy compatibility map: target_triple -> single artifact
        "artifacts": {
            art["target_triple"]: {
                "name": art["name"],
                "url": art["download_url"],
                "sha256": art["sha256"],
                "size": art["size_bytes"],
            }
            for art in artifacts
            if art["target_triple"] != "unknown"
        },
        # Complete package list preserving all artifacts across all formats per target
        "packages": [
            {
                "target_triple": art["target_triple"],
                "pkg_type": art["pkg_type"],
                "name": art["name"],
                "url": art["download_url"],
                "sha256": art["sha256"],
                "size": art["size_bytes"],
            }
            for art in artifacts
        ],
    }

    with open(args.output_latest, "w", encoding="utf-8") as f:
        json.dump(latest_payload, f, indent=2, ensure_ascii=False)
    print(f"[generate-release-manifest] Wrote updater payload to {args.output_latest}")

    # Write SHA256SUMS.txt
    if args.output_sha256sums:
        with open(args.output_sha256sums, "w", encoding="utf-8") as f:
            for art in artifacts:
                f.write(f"{art['sha256']}  {art['name']}\n")
        print(f"[generate-release-manifest] Wrote SHA256 sums to {args.output_sha256sums}")


if __name__ == "__main__":
    main()
