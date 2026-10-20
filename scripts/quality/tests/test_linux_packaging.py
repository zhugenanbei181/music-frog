"""Tests for the Linux packaging leg.

The packaging leg owns one writer for the Debian package, derives every
product identity field from the product identity registry and verifies the
built artifact against that registry. These tests pin those rules without
building the workspace: the release binary is a stand-in and the kernel asset
is not fetched.
"""

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "packaging" / "linux" / "deb"))

DEB_SPEC = importlib.util.spec_from_file_location(
    "deb_archive",
    ROOT / "packaging" / "linux" / "deb" / "deb_archive.py",
)
DEB = importlib.util.module_from_spec(DEB_SPEC)
sys.modules[DEB_SPEC.name] = DEB
DEB_SPEC.loader.exec_module(DEB)

VERIFY_SPEC = importlib.util.spec_from_file_location(
    "verify_linux_artifacts",
    ROOT / "packaging" / "linux" / "verify-linux-artifacts.py",
)
VERIFY = importlib.util.module_from_spec(VERIFY_SPEC)
sys.modules[VERIFY_SPEC.name] = VERIFY
VERIFY_SPEC.loader.exec_module(VERIFY)

from product_identity import read_properties, render_template  # noqa: E402


def identity(product: str = "iced") -> dict[str, str]:
    return read_properties()


def synthetic_tree(root: Path, product: str = "iced") -> Path:
    """A package tree that carries the registered product identity."""
    values = identity(product)
    binary = values[f"desktop.{product}.binary_name"]
    package = values[f"desktop.{product}.linux_package_id"]
    tree = root / "tree"
    (tree / "usr" / "bin").mkdir(parents=True)
    (tree / "usr" / "share" / "applications").mkdir(parents=True)
    (tree / "usr" / "share" / "icons" / "hicolor" / "512x512" / "apps").mkdir(parents=True)
    (tree / "usr" / "libexec" / "musicfrog" / binary).mkdir(parents=True)
    binary_path = tree / "usr" / "bin" / binary
    binary_path.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    binary_path.chmod(0o755)
    core = tree / "usr" / "libexec" / "musicfrog" / binary / "mihomo"
    core.write_text("kernel\n", encoding="utf-8")
    core.chmod(0o755)
    template = (ROOT / values[f"desktop.{product}.linux_desktop_file"]).read_text(encoding="utf-8")
    (tree / "usr" / "share" / "applications" / f"{package}.desktop").write_text(
        render_template(template, values), encoding="utf-8")
    icon = ROOT / values[f"desktop.{product}.icon_png"]
    (tree / "usr" / "share" / "icons" / "hicolor" / "512x512" / "apps" / f"{package}.png").write_bytes(
        icon.read_bytes())
    return tree


def control_text(product: str = "iced", version: str = "0.1.0") -> str:
    values = identity(product)
    return (f"Package: {values[f'desktop.{product}.linux_package_id']}\n"
            f"Version: {version}\n"
            "Section: net\n"
            "Priority: optional\n"
            "Architecture: amd64\n"
            "Maintainer: MusicFrog Team <dev@musicfrog.app>\n"
            "Description: test package\n")


class DebArchiveRules(unittest.TestCase):
    def test_a_package_round_trips(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = synthetic_tree(root)
            package = root / "infiltrator.deb"
            DEB.write_package(
                package,
                control_text(),
                {"postinst": b"#!/bin/sh\nexit 0\n"},
                tree,
                md5sums=DEB.build_md5sums(tree),
            )
            parsed = DEB.read_package(package)
        self.assertEqual(parsed.control["Package"], "infiltrator")
        self.assertEqual(parsed.control["Version"], "0.1.0")
        self.assertEqual(parsed.scripts, ("postinst",))
        self.assertIn("usr/bin/infiltrator-iced", parsed.data_entries)
        self.assertIn("usr/libexec/musicfrog/infiltrator-iced/mihomo", parsed.data_entries)
        self.assertIn("usr/share/applications/infiltrator.desktop", parsed.data_entries)

    def test_the_archive_layout_is_the_debian_one(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = synthetic_tree(root)
            package = root / "infiltrator.deb"
            DEB.write_package(package, control_text(), {}, tree, md5sums=DEB.build_md5sums(tree))
            members = DEB.read_ar(package)
        self.assertEqual([member.name for member in members],
                         ["debian-binary", "control.tar.gz", "data.tar.gz"])
        self.assertEqual(members[0].data, b"2.0\n")

    def test_the_same_tree_produces_the_same_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = synthetic_tree(root)
            first = root / "first.deb"
            second = root / "second.deb"
            for package in (first, second):
                DEB.write_package(package, control_text(), {}, tree,
                                  md5sums=DEB.build_md5sums(tree))
            self.assertEqual(first.read_bytes(), second.read_bytes())

    def test_the_extracted_tree_keeps_the_modes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = synthetic_tree(root)
            package = root / "infiltrator.deb"
            DEB.write_package(package, control_text(), {}, tree, md5sums=DEB.build_md5sums(tree))
            destination = root / "installed"
            DEB.extract_data(package, destination)
            binary = destination / "usr" / "bin" / "infiltrator-iced"
            self.assertTrue(os.access(binary, os.X_OK))
            self.assertTrue((destination / "usr" / "share" / "applications" /
                             "infiltrator.desktop").is_file())

    def test_the_ar_members_are_readable_by_the_system_tool(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = synthetic_tree(root)
            package = root / "infiltrator.deb"
            DEB.write_package(package, control_text(), {}, tree, md5sums=DEB.build_md5sums(tree))
            if not any(os.access(os.path.join(path, "ar"), os.X_OK)
                       for path in os.environ.get("PATH", "").split(os.pathsep)):
                self.skipTest("the ar tool is not installed")
            result = subprocess.run(["ar", "t", str(package)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.split(),
                         ["debian-binary", "control.tar.gz", "data.tar.gz"])


class LinuxArtifactIdentityRules(unittest.TestCase):
    def test_a_built_package_carries_the_registered_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = synthetic_tree(root)
            package = root / "infiltrator.deb"
            DEB.write_package(package, control_text(), {}, tree, md5sums=DEB.build_md5sums(tree))
            values = identity("iced")
            verdict = VERIFY.Verdict()
            VERIFY.verify_deb(verdict, package, values, "iced", "amd64", "0.1.0")
        failed = [finding for finding in verdict.findings if not finding.ok]
        self.assertEqual(failed, [], [finding.as_dict() for finding in failed])

    def test_a_package_with_a_foreign_identity_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = synthetic_tree(root)
            package = root / "infiltrator.deb"
            DEB.write_package(package, control_text(), {}, tree, md5sums=DEB.build_md5sums(tree))
            values = identity("iced")
            verdict = VERIFY.Verdict()
            VERIFY.verify_deb(verdict, package, values, "bevy", "amd64", "0.1.0")
        failed = [finding.check for finding in verdict.findings if not finding.ok]
        self.assertIn("control.package", failed)
        self.assertIn("binary.present", failed)
        self.assertIn("desktop.present", failed)

    def test_a_package_without_the_bundled_core_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = synthetic_tree(root)
            core = tree / "usr" / "libexec" / "musicfrog" / "infiltrator-iced" / "mihomo"
            core.unlink()
            package = root / "infiltrator.deb"
            DEB.write_package(package, control_text(), {}, tree, md5sums=DEB.build_md5sums(tree))
            values = identity("iced")
            verdict = VERIFY.Verdict()
            VERIFY.verify_deb(verdict, package, values, "iced", "amd64", "0.1.0")
        failed = [finding.check for finding in verdict.findings if not finding.ok]
        self.assertIn("core.present", failed)


class LinuxBuilderSourceRules(unittest.TestCase):
    def builders(self) -> dict[str, str]:
        return {
            "deb": (ROOT / "packaging/linux/deb/build-deb.sh").read_text(encoding="utf-8"),
            "appimage": (ROOT / "packaging/linux/appimage/build-appimage.sh").read_text(encoding="utf-8"),
        }

    def test_the_builders_read_the_identity_from_the_registry(self):
        for kind, source in self.builders().items():
            for token in (
                "scripts/product_identity.py",
                "identity_value binary_name",
                "identity_value linux_package_id",
                "identity_value linux_desktop_file",
                "identity_value icon_png",
                "scripts/render-product-template.py",
            ):
                self.assertIn(token, source, f"the {kind} builder lost {token}")

    def test_the_builders_have_no_second_identity_source(self):
        for kind, source in self.builders().items():
            for token in ("APP_BIN", "APP_PKG", "APP_DISPLAY_NAME", "APP_DESKTOP", "APP_ICON"):
                self.assertNotIn(
                    f"${{{token}:-", source,
                    f"the {kind} builder still accepts {token} from the environment")

    def test_the_linux_builder_produces_both_artifacts(self):
        source = (ROOT / "packaging/linux/build-linux.sh").read_text(encoding="utf-8")
        self.assertIn("packaging/linux/deb/build-deb.sh", source)
        self.assertIn("packaging/linux/appimage/build-appimage.sh", source)
        self.assertIn("verify-linux-artifacts.py", source)

    def test_the_install_smoke_installs_starts_and_removes(self):
        source = (ROOT / "packaging/linux/smoke-install.sh").read_text(encoding="utf-8")
        for token in ("install_deb", "install_appimage", "start_product", "remove_install",
                      "verify_installed"):
            self.assertIn(token, source)
        self.assertIn("dpkg -i", source)
        self.assertIn("dpkg --purge", source)


if __name__ == "__main__":
    unittest.main()
