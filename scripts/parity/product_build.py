#!/usr/bin/env python3
"""Immutable captured products and their actual Cargo inputs; working-tree changes are impact facts."""
from __future__ import annotations
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

PRODUCTS = {"iced": "infiltrator-iced", "bevy": "infiltrator-bevy-ui"}
RUNTIME_ROOTS = {"iced": (), "bevy": ("crates/infiltrator-bevy-widgets/assets",)}


def digest(path):
    checksum = hashlib.sha256()
    with Path(path).open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def safe_path(root, name):
    path = (root / name).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"product path escapes repository: {name}")
    return path


def dep_paths(text):
    """Cargo's aggregate binary depfile, including escaped Make paths and continuations."""
    text = text.replace("\\\n", "")
    separator = text.find(": ")
    if separator < 0:
        raise ValueError("Cargo depfile has no target separator")
    result, current, escaped = [], [], False
    for char in text[separator + 2:]:
        if escaped:
            current.append(char)
            escaped = False
        elif char == "\\":
            escaped = True
        elif char.isspace():
            if current:
                result.append("".join(current)); current = []
        else:
            current.append(char)
    if escaped:
        raise ValueError("unfinished Cargo depfile escape")
    if current:
        result.append("".join(current))
    if not result:
        raise ValueError("Cargo depfile has no build inputs")
    return result


def inputs(root, binary, surface):
    found = set()
    depfile = binary.with_suffix(".d")
    for name in dep_paths(depfile.read_text()):
        path = Path(name).resolve()
        if path.is_relative_to(root):
            found.add(path)
            for directory in path.parents:
                if not directory.is_relative_to(root):
                    break
                manifest = directory / "Cargo.toml"
                if manifest.is_file():
                    found.add(manifest)
    for name in RUNTIME_ROOTS[surface]:
        directory = root / name
        if not directory.is_dir():
            raise ValueError(f"missing declared runtime resources: {name}")
        found.update(path for path in directory.rglob("*") if path.is_file())
    toolchain = root / "rust-toolchain.toml"
    if toolchain.exists():
        found.add(toolchain)
    return {str(path.relative_to(root)): digest(path) for path in sorted(found)}


def build_id(record):
    identity = {key: record[key] for key in ("surface", "product", "binary_sha256", "inputs", "runtime_files", "rust")}
    return hashlib.sha256(json.dumps(identity, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def archive(root, surface, binary, output):
    root, binary = root.resolve(), binary.resolve()
    if surface not in PRODUCTS or binary.name != PRODUCTS[surface]:
        raise ValueError("binary does not identify the requested peer product")
    source_inputs = inputs(root, binary, surface)
    compiled_at = binary.stat().st_mtime_ns
    for name in dep_paths(binary.with_suffix(".d").read_text()):
        path = Path(name).resolve()
        if path.is_relative_to(root) and path.stat().st_mtime_ns > compiled_at:
            raise ValueError(f"compiled input changed after this artifact was produced; rebuild {surface}: {path}")
    binary_digest = digest(binary)
    record = {"schema_version": 1, "surface": surface, "product": PRODUCTS[surface],
              "binary_sha256": binary_digest, "inputs": source_inputs,
              "runtime_files": {str(Path((root / name).name) / path.relative_to(root / name)): source_inputs[str(path.relative_to(root))]
                                for name in RUNTIME_ROOTS[surface] for path in (root / name).rglob("*") if path.is_file()},
              "rust": subprocess.check_output(["rustc", "-Vv"], text=True).strip()}
    record["build_id"] = build_id(record)
    destination = root / ".evidence/builds" / record["build_id"]
    destination.mkdir(parents=True, exist_ok=True)
    frozen = destination / "binary"
    if frozen.exists():
        if digest(frozen) != binary_digest:
            raise ValueError("archived product artifact was altered")
    else:
        with tempfile.NamedTemporaryFile(prefix="binary-", suffix=".pending", dir=destination, delete=False) as target:
            temporary = Path(target.name)
            try:
                with binary.open("rb") as source:
                    shutil.copyfileobj(source, target, 1024 * 1024)
            except BaseException:
                temporary.unlink(missing_ok=True)
                raise
        try:
            temporary.chmod(0o555)
            if digest(temporary) != binary_digest or digest(binary) != binary_digest:
                raise ValueError("product binary changed while being archived")
            temporary.replace(frozen)
        finally:
            temporary.unlink(missing_ok=True)
    # The Bevy asset server reads an immutable copy, rather than the live widget directory.
    for name in RUNTIME_ROOTS[surface]:
        source = root / name
        target = destination / "runtime" / source.name
        if not target.exists():
            shutil.copytree(source, target)
        for path in source.rglob("*"):
            if path.is_file() and digest(target / path.relative_to(source)) != source_inputs[str(path.relative_to(root))]:
                raise ValueError("runtime resources changed while being archived")
    if inputs(root, binary, surface) != source_inputs:
        raise ValueError("this product's build inputs changed while being archived")
    record["binary"] = str(frozen.relative_to(root))
    record["runtime"] = str((destination / "runtime").relative_to(root))
    payload = json.dumps(record, indent=2, sort_keys=True) + "\n"
    permanent = destination / "build.json"
    if permanent.exists() and permanent.read_text() != payload:
        raise ValueError("archived build manifest differs from its content identity")
    if not permanent.exists():
        permanent.write_text(payload)
    output.write_text(payload)
    return record


def verify(root, manifest, expected_surface=None):
    record = json.loads(manifest.read_text())
    required = {"schema_version", "surface", "product", "binary_sha256", "inputs", "runtime_files", "rust", "build_id", "binary", "runtime"}
    if not isinstance(record, dict) or not required <= record.keys():
        raise ValueError("incomplete product build manifest")
    if not isinstance(record["inputs"], dict) or not record["inputs"] or not isinstance(record["runtime_files"], dict):
        raise ValueError("product build manifest lacks actual input provenance")
    if record.get("schema_version") != 1 or record.get("build_id") != build_id(record):
        raise ValueError("altered or unknown product build identity")
    if record["surface"] not in PRODUCTS or record["product"] != PRODUCTS[record["surface"]]:
        raise ValueError("invalid product identity")
    if expected_surface is not None and record["surface"] != expected_surface:
        raise ValueError("build belongs to another peer product")
    if digest(safe_path(root, record["binary"])) != record["binary_sha256"]:
        raise ValueError("altered archived product binary")
    for name, expected in record["runtime_files"].items():
        if digest(safe_path(root, str(Path(record["runtime"]) / name))) != expected:
            raise ValueError("altered archived runtime resource")
    return record


def impact(root, record):
    """Never erase a passed baseline: report which actual inputs now differ."""
    changed = []
    for name, previous in record["inputs"].items():
        path = safe_path(root, name)
        if not path.is_file() or digest(path) != previous:
            changed.append(name)
    return {"build_id": record["build_id"], "status": "review_required" if changed else "unchanged",
            "changed_inputs": sorted(changed)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--assess", type=Path, help="verify a preserved build and report current input changes without erasing its baseline")
    parser.add_argument("--surface", choices=PRODUCTS)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.assess:
        record = verify(args.root, args.assess)
        print(json.dumps({"baseline_integrity": "verified", "impact": impact(args.root, record)}, indent=2))
        return
    if not args.surface or not args.binary or not args.output:
        parser.error("archiving requires --surface, --binary and --output")
    record = archive(args.root, args.surface, args.binary, args.output)
    print(args.root / record["binary"])


if __name__ == "__main__":
    main()
