#!/usr/bin/env python3
"""Immutable captured products and their actual Cargo inputs; working-tree changes are impact facts."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

PRODUCTS = {"iced": "infiltrator-iced", "bevy": "infiltrator-bevy-ui", "slint": "infiltrator-slint"}
RUNTIME_ROOTS = {"iced": (), "bevy": ("crates/infiltrator-bevy-widgets/assets",), "slint": ()}


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


def local_package_roots(root):
    """Only Cargo-declared local packages may contribute external source inputs."""
    root = root.resolve()
    if not (root / "Cargo.toml").is_file():
        return {root}
    pending, seen, roots = [root / "Cargo.toml"], set(), {root}
    cursor = 0
    while cursor < len(pending):
        manifest = pending[cursor].resolve()
        cursor += 1
        if manifest in seen:
            continue
        seen.add(manifest)
        output = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--manifest-path", str(manifest)],
            cwd=root, check=True, text=True, capture_output=True, timeout=60,
        )
        metadata = json.loads(output.stdout)
        roots.add(Path(metadata["workspace_root"]).resolve())
        for package in metadata["packages"]:
            source = package.get("source")
            if source is not None and not source.startswith("git+"):
                continue
            package_root = Path(package["manifest_path"]).resolve().parent
            roots.add(package_root)
            # Git crates inherit the checked-out repository's workspace manifest.
            if source is not None:
                for parent in package_root.parents:
                    owner_manifest = parent / "Cargo.toml"
                    if owner_manifest.is_file():
                        roots.add(parent)
                        break
            for dependency in package["dependencies"]:
                if dependency.get("kind") in (None, "normal") and dependency.get("path"):
                    child = (Path(dependency["path"]) / "Cargo.toml").resolve()
                    if not child.is_relative_to(root) and child not in seen:
                        pending.append(child)
    return roots


def source_path(root, name, owners):
    path = (root / name).resolve()
    if not any(path.is_relative_to(owner) for owner in owners):
        raise ValueError(f"source input is outside declared Cargo packages: {name}")
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
    owners = local_package_roots(root)
    depfile = binary.with_suffix(".d")
    for name in dep_paths(depfile.read_text()):
        path = Path(name).resolve()
        owner = next((owner for owner in sorted(owners, key=lambda entry: len(entry.parts), reverse=True) if path.is_relative_to(owner)), None)
        if owner is None and path.is_relative_to(root.parent):
            raise ValueError(f"compiled sibling source is outside the current Cargo graph; rebuild: {path}")
        if owner is not None:
            if path.is_file():
                found.add(path)
            elif path.is_dir():
                found.update(entry for entry in path.rglob("*") if entry.is_file())
            for directory in path.parents:
                manifest = directory / "Cargo.toml"
                if directory in owners and manifest.is_file():
                    found.add(manifest)
    for name in RUNTIME_ROOTS[surface]:
        directory = root / name
        if not directory.is_dir():
            raise ValueError(f"missing declared runtime resources: {name}")
        found.update(path for path in directory.rglob("*") if path.is_file())
    for name in ("Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml", ".cargo/config"):
        configuration = root / name
        if configuration.is_file():
            found.add(configuration)
    return {Path(os.path.relpath(path, root)).as_posix(): digest(path) for path in sorted(found)}


def build_id(record):
    identity = {key: record[key] for key in ("surface", "product", "binary_sha256", "inputs", "runtime_files", "rust")}
    return hashlib.sha256(json.dumps(identity, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def archive(root, surface, binary, output):
    root, binary = root.resolve(), binary.resolve()
    if surface not in PRODUCTS or binary.name != PRODUCTS[surface]:
        raise ValueError("binary does not identify the requested peer product")
    source_inputs = inputs(root, binary, surface)
    compiled_at = binary.stat().st_mtime_ns
    owners = local_package_roots(root)
    for name in dep_paths(binary.with_suffix(".d").read_text()):
        path = Path(name).resolve()
        if any(path.is_relative_to(owner) for owner in owners) and path.stat().st_mtime_ns > compiled_at:
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
    owners = local_package_roots(root)
    for name, previous in record["inputs"].items():
        path = source_path(root, name, owners)
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
