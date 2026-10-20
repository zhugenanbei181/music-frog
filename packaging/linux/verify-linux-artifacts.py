#!/usr/bin/env python3
"""Verify that a built Linux artifact carries the registered product identity.

The product identity registry (`product-identity.properties`) is the single
source for the package id, the binary name, the display name, the icon and
the desktop entry. This verifier reads the *built artifact* and compares it
against that registry, so a packaging script that drifts from the registry
fails here instead of shipping an artifact with the wrong identity.

The same checks run against three shapes of the artifact:

  * a Debian package (`.deb`),
  * an extracted AppImage (the squashfs payload of an `.AppImage`),
  * an installed tree on disk (the staged install of the install smoke).

Usage:
    python3 packaging/linux/verify-linux-artifacts.py --product iced --arch x86_64 \\
        --deb dist/infiltrator_0.1.0_amd64.deb --appimage dist/Infiltrator-Linux-x86_64.AppImage
    python3 packaging/linux/verify-linux-artifacts.py --product iced \\
        --installed-root target/.../staged-root

Exit codes: 0 every check passed, 1 an identity mismatch was found, 2 the
arguments or the artifact could not be read.
"""

from __future__ import annotations

import argparse
import json
import os
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from product_identity import read_properties  # noqa: E402  (the registry reader lives in scripts/)

DEB_ARCHITECTURES = {"x86_64": "amd64", "amd64": "amd64", "aarch64": "arm64", "arm64": "arm64"}


@dataclass(frozen=True)
class Layout:
    """Where a packaging layout keeps the product files, relative to its root."""

    binary: str
    desktop: str
    icon: str
    core: str
    launcher: str | None = None


@dataclass
class Finding:
    """One identity check and its verdict."""

    artifact: str
    check: str
    ok: bool
    detail: str = ""

    def as_dict(self) -> dict[str, object]:
        return {"artifact": self.artifact, "check": self.check, "ok": self.ok, "detail": self.detail}


@dataclass
class Verdict:
    """The collected findings of one verification run."""

    findings: list[Finding] = field(default_factory=list)

    def add(self, artifact: str, check: str, ok: bool, detail: str = "") -> None:
        self.findings.append(Finding(artifact=artifact, check=check, ok=ok, detail=detail))

    @property
    def ok(self) -> bool:
        return all(finding.ok for finding in self.findings)

    def as_dict(self) -> dict[str, object]:
        return {
            "ok": self.ok,
            "checks": [finding.as_dict() for finding in self.findings],
        }


def identity_for(product: str) -> dict[str, str]:
    """Read the registered identity of one desktop product."""
    values = read_properties()
    prefix = f"desktop.{product}"
    required = (
        "binary_name",
        "linux_package_id",
        "display_name",
        "icon_png",
        "linux_desktop_file",
        "release_artifact_prefix",
    )
    for key in required:
        if f"{prefix}.{key}" not in values:
            raise ValueError(f"the identity registry has no {prefix}.{key}")
    return values


def desktop_layout(values: dict[str, str], product: str) -> Layout:
    """The file layout a Debian package installs."""
    binary = values[f"desktop.{product}.binary_name"]
    package = values[f"desktop.{product}.linux_package_id"]
    return Layout(
        binary=f"usr/bin/{binary}",
        desktop=f"usr/share/applications/{package}.desktop",
        icon=f"usr/share/icons/hicolor/512x512/apps/{package}.png",
        core=f"usr/libexec/musicfrog/{binary}/mihomo",
    )


def appimage_layout(values: dict[str, str], product: str) -> Layout:
    """The file layout an AppImage carries."""
    binary = values[f"desktop.{product}.binary_name"]
    package = values[f"desktop.{product}.linux_package_id"]
    return Layout(
        binary=f"usr/bin/{binary}",
        desktop=f"usr/share/applications/{package}.desktop",
        icon=f"usr/share/icons/hicolor/512x512/apps/{package}.png",
        core="usr/bin/mihomo",
        launcher="AppRun",
    )


def directory_tree(root: Path) -> dict[str, tuple[bytes | None, int]]:
    """Read a directory tree into `path -> (content, mode)` pairs."""
    tree: dict[str, tuple[bytes | None, int]] = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root).as_posix()
        mode = path.lstat().st_mode
        if path.is_symlink():
            tree[relative] = (None, stat.S_IFLNK | 0o777)
        elif path.is_dir():
            tree[relative] = (None, stat.S_IFDIR | 0o755)
        else:
            tree[relative] = (path.read_bytes(), stat.S_IMODE(mode))
    return tree


def deb_tree(package: Path) -> tuple[dict[str, tuple[bytes | None, int]], dict[str, str]]:
    """Read a Debian package into its file tree and its control fields."""
    sys.path.insert(0, str(REPO_ROOT / "packaging/linux/deb"))
    from deb_archive import extract_data  # noqa: PLC0415  (the writer lives with the builder)

    with tempfile.TemporaryDirectory(prefix="infiltrator-deb-verify-") as temporary:
        destination = Path(temporary)
        package_info = extract_data(package, destination)
        return directory_tree(destination), package_info.control


def appimage_tree(appimage: Path) -> dict[str, tuple[bytes | None, int]]:
    """Extract an AppImage and read the file tree it carries."""
    with tempfile.TemporaryDirectory(prefix="infiltrator-appimage-verify-") as temporary:
        workspace = Path(temporary)
        result = subprocess.run(
            [str(appimage), "--appimage-extract"],
            cwd=workspace,
            capture_output=True,
            text=True,
            check=False,
            env={**os.environ, "APPIMAGE_EXTRACT_AND_RUN": "1"},
        )
        root = workspace / "squashfs-root"
        if result.returncode != 0 or not root.is_dir():
            raise ValueError(
                f"{appimage}: the AppImage payload cannot be extracted: "
                f"{result.stderr.strip() or result.stdout.strip()}"
            )
        return directory_tree(root)


def _parse_desktop(text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or stripped.startswith("["):
            continue
        key, separator, value = stripped.partition("=")
        if separator:
            fields[key.strip()] = value.strip()
    return fields


def check_tree(
    verdict: Verdict,
    artifact: str,
    tree: dict[str, tuple[bytes | None, int]],
    layout: Layout,
    values: dict[str, str],
    product: str,
    control: dict[str, str] | None = None,
    version: str | None = None,
) -> None:
    """Compare an installed tree against the registered product identity."""
    binary = values[f"desktop.{product}.binary_name"]
    package = values[f"desktop.{product}.linux_package_id"]
    display_name = values[f"desktop.{product}.display_name"]

    if control is not None:
        verdict.add(artifact, "control.package", control.get("Package") == package,
                    f"expected {package!r}, found {control.get('Package')!r}")
        expected_version = version or values["version.name"]
        verdict.add(artifact, "control.version", control.get("Version") == expected_version,
                    f"expected {expected_version!r}, found {control.get('Version')!r}")

    entry = tree.get(layout.binary)
    verdict.add(artifact, "binary.present", entry is not None, layout.binary)
    if entry is not None:
        verdict.add(artifact, "binary.executable", bool(entry[1] & 0o111), oct(entry[1]))

    entry = tree.get(layout.core)
    verdict.add(artifact, "core.present", entry is not None, layout.core)
    if entry is not None:
        verdict.add(artifact, "core.executable", bool(entry[1] & 0o111), oct(entry[1]))

    entry = tree.get(layout.desktop)
    verdict.add(artifact, "desktop.present", entry is not None, layout.desktop)
    if entry is not None and entry[0] is not None:
        fields = _parse_desktop(entry[0].decode("utf-8", "replace"))
        verdict.add(artifact, "desktop.name", fields.get("Name") == display_name,
                    f"expected {display_name!r}, found {fields.get('Name')!r}")
        verdict.add(artifact, "desktop.exec", fields.get("Exec", "").split()[0:1] == [binary],
                    f"expected {binary!r}, found {fields.get('Exec')!r}")
        verdict.add(artifact, "desktop.icon", fields.get("Icon") == package,
                    f"expected {package!r}, found {fields.get('Icon')!r}")

    entry = tree.get(layout.icon)
    verdict.add(artifact, "icon.present", entry is not None, layout.icon)
    if entry is not None and entry[0] is not None:
        expected = (REPO_ROOT / values[f"desktop.{product}.icon_png"]).read_bytes()
        verdict.add(artifact, "icon.matches_registry", entry[0] == expected,
                    f"expected the registered icon {values[f'desktop.{product}.icon_png']}")

    if layout.launcher is not None:
        entry = tree.get(layout.launcher)
        verdict.add(artifact, "launcher.present", entry is not None, layout.launcher)
        if entry is not None:
            verdict.add(artifact, "launcher.executable", bool(entry[1] & 0o111), oct(entry[1]))
            if entry[0] is not None:
                launcher = entry[0].decode("utf-8", "replace")
                verdict.add(artifact, "launcher.executes_binary", f"usr/bin/{binary}" in launcher,
                            f"the launcher must exec usr/bin/{binary}")


def verify_deb(
    verdict: Verdict,
    package: Path,
    values: dict[str, str],
    product: str,
    architecture: str,
    version: str | None = None,
) -> None:
    """Verify a Debian package against the registered product identity."""
    tree, control = deb_tree(package)
    expected_architecture = DEB_ARCHITECTURES.get(architecture, architecture)
    verdict.add(str(package), "control.architecture",
                control.get("Architecture") == expected_architecture,
                f"expected {expected_architecture!r}, found {control.get('Architecture')!r}")
    check_tree(verdict, str(package), tree, desktop_layout(values, product), values, product,
               control, version)


def verify_appimage(
    verdict: Verdict,
    appimage: Path,
    values: dict[str, str],
    product: str,
) -> None:
    """Verify an AppImage against the registered product identity."""
    tree = appimage_tree(appimage)
    check_tree(verdict, str(appimage), tree, appimage_layout(values, product), values, product)


def selected_tree(root: Path, relatives: list[str]) -> dict[str, tuple[bytes | None, int]]:
    """Read exactly the listed paths of a tree, without walking it.

    A system install has the filesystem root as its root; walking it would
    read the whole machine, so only the paths the layout names are read.
    """
    tree: dict[str, tuple[bytes | None, int]] = {}
    for relative in relatives:
        path = root / relative
        if not path.exists() and not path.is_symlink():
            continue
        mode = path.lstat().st_mode
        if path.is_symlink():
            tree[relative] = (None, stat.S_IFLNK | 0o777)
        elif path.is_dir():
            tree[relative] = (None, stat.S_IFDIR | 0o755)
        else:
            tree[relative] = (path.read_bytes(), stat.S_IMODE(mode))
    return tree


def layout_paths(layout: Layout) -> list[str]:
    """The paths one layout names."""
    paths = [layout.binary, layout.desktop, layout.icon, layout.core]
    if layout.launcher is not None:
        paths.append(layout.launcher)
    return paths


def verify_installed_root(
    verdict: Verdict,
    root: Path,
    values: dict[str, str],
    product: str,
    layout_name: str = "deb",
) -> None:
    """Verify an installed tree against the registered product identity."""
    layout = appimage_layout(values, product) if layout_name == "appimage" \
        else desktop_layout(values, product)
    tree = selected_tree(root, layout_paths(layout))
    check_tree(verdict, str(root), tree, layout, values, product)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", required=True, help="the registered desktop product")
    parser.add_argument("--arch", default="x86_64", help="the target architecture")
    parser.add_argument("--deb", type=Path, help="the Debian package to verify")
    parser.add_argument("--appimage", type=Path, help="the AppImage to verify")
    parser.add_argument("--installed-root", type=Path, help="an installed tree to verify")
    parser.add_argument("--layout", choices=("deb", "appimage"), default="deb",
                        help="the file layout of the installed tree")
    parser.add_argument("--version", help="the expected package version (default: the registry version)")
    parser.add_argument("--json", type=Path, help="write the verdict as JSON")
    arguments = parser.parse_args()

    try:
        values = identity_for(arguments.product)
    except (OSError, ValueError) as error:
        print(f"linux-artifact-verify: {error}", file=sys.stderr)
        return 2

    verdict = Verdict()
    try:
        if arguments.deb is not None:
            verify_deb(verdict, arguments.deb, values, arguments.product, arguments.arch,
                       arguments.version)
        if arguments.appimage is not None:
            verify_appimage(verdict, arguments.appimage, values, arguments.product)
        if arguments.installed_root is not None:
            verify_installed_root(verdict, arguments.installed_root, values, arguments.product,
                                  arguments.layout)
    except (OSError, ValueError, KeyError) as error:
        print(f"linux-artifact-verify: {error}", file=sys.stderr)
        return 2

    if arguments.json is not None:
        arguments.json.parent.mkdir(parents=True, exist_ok=True)
        arguments.json.write_text(json.dumps(verdict.as_dict(), ensure_ascii=False, indent=2),
                                  encoding="utf-8")
    for finding in verdict.findings:
        marker = "ok  " if finding.ok else "FAIL"
        print(f"[{marker}] {finding.artifact} :: {finding.check} :: {finding.detail}")
    if not verdict.ok:
        failed = [finding for finding in verdict.findings if not finding.ok]
        print(f"linux-artifact-verify: FAIL ({len(failed)} identity mismatches)", file=sys.stderr)
        return 1
    print(f"linux-artifact-verify: OK ({len(verdict.findings)} checks)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
