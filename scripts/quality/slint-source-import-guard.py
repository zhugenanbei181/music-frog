#!/usr/bin/env python3
"""Verify the verbatim Lumen source import and optional source ZIP parity."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
import zipfile


ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE_ROOT = ROOT / "crates/infiltrator-slint/lumen-source"
MANIFEST = SOURCE_ROOT / "SOURCE_IMPORT_MANIFEST.json"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--archive",
        type=pathlib.Path,
        help="also compare every imported source file with the supplied ZIP",
    )
    args = parser.parse_args()

    try:
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"invalid Lumen source import manifest: {error}", file=sys.stderr)
        return 1

    files = manifest.get("files")
    if not isinstance(files, dict) or manifest.get("imported_files") != len(files):
        print("manifest file count does not match its source records", file=sys.stderr)
        return 1

    errors: list[str] = []
    total_bytes = 0
    for original, record in files.items():
        destination = record.get("path", "")
        if not isinstance(destination, str):
            errors.append(f"{original}: invalid destination path")
            continue
        source_path = (SOURCE_ROOT / destination).resolve()
        if not source_path.is_relative_to(SOURCE_ROOT.resolve()):
            errors.append(f"{original}: destination escapes the Lumen source directory")
            continue
        try:
            data = source_path.read_bytes()
        except OSError:
            errors.append(f"{original}: missing preserved file {destination}")
            continue
        total_bytes += len(data)
        if record.get("patched"):
            if not record.get("patch_source"):
                errors.append(f"{original}: patched file missing patch_source provenance")
            if not record.get("original_sha256") or not record.get("original_bytes"):
                errors.append(f"{original}: patched file missing original archive hashes")
        if len(data) != record.get("bytes") or sha256(data) != record.get("sha256"):
            errors.append(f"{original}: preserved bytes do not match the manifest")

    if total_bytes != manifest.get("imported_bytes"):
        errors.append("manifest byte count does not match the preserved source files")

    if args.archive is not None:
        try:
            archive_bytes = args.archive.read_bytes()
            with zipfile.ZipFile(args.archive) as archive:
                archive_files = {
                    item.filename
                    for item in archive.infolist()
                    if not item.is_dir() and not item.filename.startswith(".git/")
                }
                if sha256(archive_bytes) != manifest.get("archive_sha256"):
                    errors.append("supplied ZIP hash does not match the recorded archive")
                if archive_files != set(files):
                    errors.append("supplied ZIP file paths do not match the import manifest")
                for original, record in files.items():
                    if original in archive_files:
                        data = archive.read(original)
                        expected_bytes = record.get("original_bytes", record.get("bytes"))
                        expected_sha = record.get("original_sha256", record.get("sha256"))
                        if len(data) != expected_bytes or sha256(data) != expected_sha:
                            errors.append(f"{original}: source ZIP bytes do not match the manifest")
        except (OSError, zipfile.BadZipFile) as error:
            errors.append(f"cannot read supplied source ZIP: {error}")

    if errors:
        print("Slint source import integrity failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    patched_count = sum(1 for r in files.values() if r.get("patched"))
    print(
        "Slint source import integrity passed: "
        f"{len(files)} files ({patched_count} patched with provenance), {total_bytes} bytes"
        + ("; supplied ZIP matches" if args.archive is not None else "")
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
