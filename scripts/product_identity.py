#!/usr/bin/env python3
"""Read and validate the cross-platform product identity registry."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
IDENTITY = ROOT / "product-identity.properties"
VERSION_PATTERN = re.compile(r"^(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$")
PRODUCT_PATTERN = re.compile(r"^[a-z][a-z0-9_-]*$")
PLACEHOLDER_PATTERN = re.compile(r"@([a-z][a-z0-9_.-]*)@")


def read_properties(path: Path = IDENTITY) -> dict[str, str]:
    values: dict[str, str] = {}
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        key, separator, value = stripped.partition("=")
        if not separator or not key.strip() or not value.strip():
            raise ValueError(f"{path}:{line_number}: expected key=value")
        normalized_key = key.strip()
        if normalized_key in values:
            raise ValueError(f"{path}:{line_number}: duplicate key {normalized_key}")
        values[normalized_key] = value.strip()
    if "version.name" in values:
        raise ValueError(f"{path}: version.name belongs only in Cargo.toml [workspace.package]")
    workspace_path = path.resolve().parent / "Cargo.toml"
    workspace = tomllib.loads(workspace_path.read_text(encoding="utf-8"))
    version = workspace.get("workspace", {}).get("package", {}).get("version")
    if not isinstance(version, str) or not version:
        raise ValueError(f"{workspace_path}: missing [workspace.package].version")
    values["version.name"] = version
    return values


def version_parts(values: dict[str, str]) -> tuple[int, int, int]:
    match = VERSION_PATTERN.fullmatch(values.get("version.name", ""))
    if match is None:
        raise ValueError("version.name must be a SemVer major.minor.patch string")
    parts = tuple(int(part) for part in match.groups())
    if any(part > 255 for part in parts):
        raise ValueError("version components must fit Android packed-code 8-bit fields")
    return parts


def product_ids(values: dict[str, str], surface: str = "android") -> tuple[str, ...]:
    products = tuple(
        product.strip()
        for product in values.get(f"{surface}.products", "").split(",")
        if product.strip()
    )
    if not products:
        raise ValueError(f"{surface}.products must list every independently installable app")
    if len(set(products)) != len(products):
        raise ValueError("products must not contain duplicate IDs")
    if any(PRODUCT_PATTERN.fullmatch(product) is None for product in products):
        raise ValueError("product IDs must use lowercase letters, digits, '-' or '_'")
    return products


def get_value(values: dict[str, str], key: str) -> str:
    major, minor, patch = version_parts(values)
    if key.endswith(".version_code"):
        product = key.removesuffix(".version_code")
        if product not in product_ids(values):
            raise ValueError(f"unknown Android product identity key: {key}")
        scheme = values.get(f"{product}.version_code_scheme", "")
        if scheme == "android-composite":
            return str(major * 1_000_000 + minor * 1_000 + patch)
        if scheme == "android-packed":
            try:
                product_slot = int(values[f"{product}.version_code_slot"])
            except (KeyError, ValueError) as error:
                raise ValueError(f"{product}.version_code_slot must be an integer") from error
            if not 0 <= product_slot <= 255:
                raise ValueError(f"{product}.version_code_slot must fit an 8-bit field")
            return str((product_slot << 24) | (major << 16) | (minor << 8) | patch)
        raise ValueError(f"unknown version-code scheme for {product}: {scheme!r}")
    try:
        return values[key]
    except KeyError as error:
        raise ValueError(f"unknown product identity key: {key}") from error


def render_template(template: str, values: dict[str, str]) -> str:
    def replace(match: re.Match[str]) -> str:
        try:
            return get_value(values, match.group(1))
        except ValueError as error:
            raise ValueError(f"unknown product identity placeholder {match.group(0)}") from error

    return PLACEHOLDER_PATTERN.sub(replace, template)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--get", required=True, help="identity key to print")
    arguments = parser.parse_args()
    try:
        print(get_value(read_properties(), arguments.get))
    except (OSError, ValueError, KeyError) as error:
        print(f"product identity: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
