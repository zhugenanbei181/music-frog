#!/usr/bin/env python3
"""Fresh, process-bound real pixels with hashes and measured image geometry."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import re
import subprocess
from pathlib import Path
from rendered_frame import compare_rendered_frame
import product_build

ROOT = Path(__file__).resolve().parents[2]
VIEWPORTS = {"standard": (1180, 780), "compact": (720, 480)}


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def file_identity(path):
    stat = Path(path).stat()
    return stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns


def source_fingerprint(root):
    output = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
                             "crates", "Cargo.toml", "Cargo.lock", "scripts"], cwd=root,
                            check=True, stdout=subprocess.PIPE).stdout
    digest = hashlib.sha256()
    for name in sorted(set(output.decode().split("\0")) - {""}):
        path = root / name
        if not path.is_file():
            continue
        digest.update(name.encode() + b"\0" + path.read_bytes() + b"\0")
    return digest.hexdigest()


def pixel_signature(path, bounds=None):
    from PIL import Image, ImageFilter, ImageStat
    with Image.open(path) as image:
        if image.format != "PNG" or image.width > 4096 or image.height > 4096:
            raise ValueError(f"invalid capture image: {path}")
        if bounds is not None:
            x, y, width, height = bounds
            image = image.crop((math.floor(x), math.floor(y), math.ceil(x + width), math.ceil(y + height)))
        gray = image.convert("RGB").convert("L")
        deviation = ImageStat.Stat(gray).stddev[0]
        grid = gray.resize((16, 12)).tobytes()
        edges = gray.filter(ImageFilter.FIND_EDGES).crop((1, 1, gray.width - 1, gray.height - 1))
        bbox = edges.point(lambda value: 255 if value > 24 else 0).getbbox()
        if deviation < 2 or bbox is None:
            raise ValueError(f"blank capture pixels: {path}")
        return {"width": image.width, "height": image.height, "edge_bbox": list(bbox),
                "luma_grid_sha256": hashlib.sha256(bytes(grid)).hexdigest(),
                "luma_stddev": round(deviation, 4)}



def interaction_bounds(marker, feature, viewport):
    expected = f"scenario={feature} activated=true"
    lines = [line for line in marker.splitlines() if expected in line]
    if len(lines) != 1 or not (match := re.search(r"\bbounds=([^\s]+)", lines[0])):
        raise ValueError(f"missing native interaction bounds: {feature}/{viewport}")
    try:
        bounds = [float(value) for value in match[1].split(",")]
    except ValueError as error:
        raise ValueError("invalid native interaction bounds") from error
    if len(bounds) != 4 or not all(math.isfinite(value) for value in bounds):
        raise ValueError("invalid native interaction bounds")
    x, y, width, height = bounds
    screen_width, screen_height = VIEWPORTS[viewport]
    if x < 0 or y < 0 or width <= 0 or height <= 0 or x + width > screen_width or y + height > screen_height:
        raise ValueError(f"interaction is outside the captured viewport: {feature}/{viewport}")
    return bounds


def interaction_signature(path, bounds):
    return pixel_signature(path, bounds)

def repo_path(root, raw):
    path = (root / raw).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"receipt path escapes repository: {raw}")
    return path


def build_receipts(root, runs):
    receipts = []
    for surface, run in runs.items():
        metadata = dict(line.split("=", 1) for line in (run / "capture-metadata.txt").read_text().splitlines() if "=" in line)
        build_manifest = repo_path(root, metadata["build_manifest"])
        product = product_build.verify(root, build_manifest, surface)
        binary = repo_path(root, product["binary"])
        if metadata.get("binary_sha256") != sha256(binary):
            raise ValueError(f"{surface}: binary differs from the captured process")
        with (run / "capture-manifest.tsv").open() as file:
            rows = list(csv.DictReader(file, delimiter="\t"))
        for row in rows:
            scenario = row["scenario"]
            feature, separator, viewport = scenario.rpartition("-")
            if not separator or viewport not in VIEWPORTS or row["status"] != "ok":
                raise ValueError(f"{surface}: failed or non-interaction capture {scenario}")
            marker = run / scenario / "marker.log"
            expected_marker = f"scenario={feature} activated=true"
            if expected_marker not in marker.read_text():
                raise ValueError(f"{surface}: route-only capture cannot prove {scenario}")
            bounds = interaction_bounds(marker.read_text(), feature, viewport)
            image = run / scenario / "image.png"
            signature = pixel_signature(image)
            region_signature = interaction_signature(image, bounds)
            frame_proof = {}
            if surface == "iced":
                rendered, mismatch = compare_rendered_frame(image, bounds)
                frame_proof = {"rendered_frame": str(rendered.relative_to(root)),
                               "rendered_frame_sha256": sha256(rendered), "rendered_frame_mismatch": mismatch}
            expected_size = VIEWPORTS[viewport]
            if (signature["width"], signature["height"]) != expected_size:
                raise ValueError(f"{surface}: wrong viewport for {scenario}: {signature}")
            if row["sha256"] != sha256(image) or int(row["app_pid"]) <= 0 or not row["window_id"]:
                raise ValueError(f"{surface}: invalid process/window/image receipt: {scenario}")
            receipts.append({"feature_id": feature, "surface": surface, "viewport": viewport,
                             "image": str(image.relative_to(root)), "sha256": sha256(image),
                             "marker": str(marker.relative_to(root)), "marker_sha256": sha256(marker), "pixel_signature": signature,
                             "interaction_bounds": bounds, "interaction_pixel_signature": region_signature,
                             "app_pid": int(row["app_pid"]), "window_id": row["window_id"],
                             "binary_sha256": metadata["binary_sha256"], "source_fingerprint": metadata.get("source_fingerprint"),
                             "build_id": product["build_id"], "build_manifest": str(build_manifest.relative_to(root)),
                             "build_manifest_sha256": sha256(build_manifest),
                             "capture_manifest": str((run / "capture-manifest.tsv").relative_to(root)),
                             "capture_manifest_sha256": sha256(run / "capture-manifest.tsv"),
                             "capture_metadata_sha256": sha256(run / "capture-metadata.txt"), **frame_proof})
    return {"schema_version": 4, "receipts": receipts}


def verified_receipts(path, root, selected_builds=None):
    document = json.loads(Path(path).read_text())
    if document.get("schema_version") not in (3, 4) or not document.get("receipts"):
        raise ValueError("empty or unknown visual receipt schema")
    verified = set()
    binary_digests = {}
    for row in document["receipts"]:
        key = (row["feature_id"], row["surface"], row["viewport"])
        if key in verified or row["surface"] not in {"iced", "bevy"} or row["viewport"] not in VIEWPORTS:
            raise ValueError(f"duplicate/invalid pixel identity: {key}")
        image = repo_path(root, row["image"])
        if sha256(image) != row["sha256"] or pixel_signature(image) != row["pixel_signature"]:
            raise ValueError(f"altered capture pixels or geometry: {key}")
        if (row["pixel_signature"]["width"], row["pixel_signature"]["height"]) != VIEWPORTS[row["viewport"]]:
            raise ValueError(f"wrong capture dimensions: {key}")
        marker = repo_path(root, row["marker"]).read_text()
        if f"scenario={row['feature_id']} activated=true" not in marker:
            raise ValueError(f"interaction was not activated: {key}")
        if sha256(repo_path(root, row["marker"])) != row["marker_sha256"]:
            raise ValueError(f"altered activation marker: {key}")
        bounds = interaction_bounds(marker, row["feature_id"], row["viewport"])
        if row.get("interaction_bounds") != bounds or interaction_signature(image, bounds) != row.get("interaction_pixel_signature"):
            raise ValueError(f"altered interaction pixels or native geometry: {key}")
        manifest = repo_path(root, row["capture_manifest"])
        with manifest.open() as file:
            matches = [entry for entry in csv.DictReader(file, delimiter="\t")
                       if entry["scenario"] == f"{row['feature_id']}-{row['viewport']}"]
        if len(matches) != 1 or matches[0]["status"] != "ok" or matches[0]["sha256"] != row["sha256"] \
                or matches[0]["app_pid"] != str(row["app_pid"]) or matches[0]["window_id"] != row["window_id"]:
            raise ValueError(f"process/window receipt mismatch: {key}")
        metadata_path = manifest.parent / "capture-metadata.txt"
        metadata = dict(line.split("=", 1) for line in metadata_path.read_text().splitlines() if "=" in line)
        if metadata.get("source_fingerprint") != row["source_fingerprint"] or metadata.get("binary_sha256") != row["binary_sha256"]:
            raise ValueError(f"capture metadata mismatch: {key}")
        if sha256(metadata_path) != row["capture_metadata_sha256"] or sha256(manifest) != row["capture_manifest_sha256"]:
            raise ValueError(f"altered capture metadata or manifest: {key}")
        scenario_directory = manifest.parent / f"{row['feature_id']}-{row['viewport']}"
        if image != (scenario_directory / "image.png").resolve() or repo_path(root, row["marker"]) != (scenario_directory / "marker.log").resolve():
            raise ValueError(f"pixels or activation marker belong to another capture: {key}")
        if row["surface"] == "iced":
            rendered, mismatch = compare_rendered_frame(image, bounds)
            if repo_path(root, row.get("rendered_frame", "")) != rendered or row.get("rendered_frame_sha256") != sha256(rendered) \
                    or row.get("rendered_frame_mismatch") != mismatch:
                raise ValueError(f"altered or missing native frame proof: {key}")
        if row["app_pid"] <= 0 or not row["window_id"]:
            raise ValueError(f"invalid capture process/window: {key}")
        if document["schema_version"] == 4:
            manifest_path = repo_path(root, row["build_manifest"])
            if sha256(manifest_path) != row["build_manifest_sha256"]:
                raise ValueError(f"altered product build manifest: {key}")
            product = product_build.verify(root, manifest_path, row["surface"])
            if selected_builds is not None and selected_builds.get(row["surface"]) != product["build_id"]:
                raise ValueError(f"receipt needs revalidation for the selected product build: {key}")
            if product["build_id"] != row["build_id"] or product["binary_sha256"] != row["binary_sha256"]:
                raise ValueError(f"product build receipt mismatch: {key}")
            if metadata.get("build_manifest") != row["build_manifest"] or metadata.get("binary") != product["binary"]:
                raise ValueError(f"captured executable differs from archived product: {key}")
            binary = repo_path(root, product["binary"])
        else:
            # Old receipts remain baseline evidence; a missing original binary cannot be invented.
            if selected_builds is not None:
                raise ValueError(f"legacy receipt lacks an archived build identity for release selection: {key}")
            binary = root / "target/debug" / ("infiltrator-iced" if row["surface"] == "iced" else "infiltrator-bevy-ui")
        if binary not in binary_digests:
            identity = file_identity(binary)
            digest = sha256(binary)
            if file_identity(binary) != identity:
                raise ValueError(f"capture binary changed during verification: {key}")
            binary_digests[binary] = identity, digest
        if binary_digests[binary][1] != row["binary_sha256"]:
            raise ValueError(f"stale capture binary: {key}")
        verified.add(key)
    for binary, (identity, _) in binary_digests.items():
        if file_identity(binary) != identity:
            raise ValueError("capture binary changed during verification")
    return verified


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fingerprint", action="store_true")
    parser.add_argument("--iced-run", type=Path)
    parser.add_argument("--bevy-run", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / ".evidence/visual_receipts.json")
    args = parser.parse_args()
    if args.fingerprint:
        print(source_fingerprint(ROOT))
        return
    if not args.iced_run or not args.bevy_run:
        parser.error("both --iced-run and --bevy-run are required")
    document = build_receipts(ROOT, {"iced": args.iced_run.resolve(), "bevy": args.bevy_run.resolve()})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2) + "\n")
    verified = verified_receipts(args.output, ROOT)
    print(f"verified {len(verified)} process-bound pixel receipts: {args.output}")


if __name__ == "__main__":
    main()
