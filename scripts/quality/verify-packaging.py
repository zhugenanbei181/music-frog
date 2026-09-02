#!/usr/bin/env python3
"""Packaging Template and Release Asset Validation Guard.

Validates syntax, structure, metadata, and specification compliance across all
platform packaging configurations:
- Windows: NSIS script (.nsi), WiX installer (.wxs)
- Linux: AppImage (build script, AppRun, .desktop entry), Debian package (control, maintainer scripts)
- macOS: DMG creation script, Info.plist, entitlements.plist
- Release Tooling: generate-release-manifest.py and signing automation

Usage:
    python3 scripts/quality/verify-packaging.py [--verbose]
"""

from __future__ import annotations

import argparse
import os
import pathlib
import plistlib
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]


class ValidationResult:
    def __init__(self, target: str):
        self.target = target
        self.errors: list[str] = []
        self.warnings: list[str] = []

    def error(self, msg: str) -> None:
        self.errors.append(msg)

    def warning(self, msg: str) -> None:
        self.warnings.append(msg)

    @property
    def is_ok(self) -> bool:
        return len(self.errors) == 0


def check_bash_syntax(script_path: pathlib.Path) -> list[str]:
    """Checks bash script syntax using bash -n."""
    errors = []
    if not script_path.exists():
        return [f"Script not found: {script_path}"]

    try:
        proc = subprocess.run(
            ["bash", "-n", str(script_path)],
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode != 0:
            errors.append(f"Bash syntax error: {proc.stderr.strip()}")
    except Exception as e:
        errors.append(f"Failed to invoke bash -n: {e}")
    return errors


def validate_nsis_script(nsi_path: pathlib.Path) -> ValidationResult:
    """Validates NSIS installer script structure and syntax."""
    res = ValidationResult(str(nsi_path.relative_to(REPO_ROOT)))
    if not nsi_path.is_file():
        res.error("File does not exist")
        return res

    content = nsi_path.read_text(encoding="utf-8", errors="replace")

    # 1. Required Header Includes
    required_includes = ["MUI2.nsh", "x64.nsh", "FileFunc.nsh"]
    for inc in required_includes:
        if not re.search(rf'!include\s+["\']?{re.escape(inc)}["\']?', content):
            res.error(f"Missing required include: {inc}")

    # 2. Section & Macro Block Balance
    section_starts = len(re.findall(r'^\s*Section\b', content, re.MULTILINE))
    section_ends = len(re.findall(r'^\s*SectionEnd\b', content, re.MULTILINE))
    if section_starts != section_ends:
        res.error(f"Unbalanced Section/SectionEnd: {section_starts} start(s) vs {section_ends} end(s)")

    if_starts = len(re.findall(r'^\s*\$\{(?:If|IfNot)\b', content, re.MULTILINE))
    if_ends = len(re.findall(r'^\s*\$\{(?:EndIf|EndIfNot)\b', content, re.MULTILINE))
    if if_starts != if_ends:
        res.error(f"Unbalanced ${{If}}/${{EndIf}}: {if_starts} start(s) vs {if_ends} end(s)")

    # 3. Essential Directives
    essential_directives = [
        r'Name\s+["\']',
        r'OutFile\s+["\']',
        r'InstallDir\s+["\']',
        r'RequestExecutionLevel\s+admin',
        r'SetCompressor\s+',
        r'!insertmacro\s+MUI_PAGE_WELCOME',
        r'!insertmacro\s+MUI_PAGE_INSTFILES',
        r'!insertmacro\s+MUI_PAGE_FINISH',
        r'!insertmacro\s+MUI_UNPAGE_CONFIRM',
        r'!insertmacro\s+MUI_UNPAGE_INSTFILES',
        r'!insertmacro\s+MUI_LANGUAGE\s+"English"',
    ]
    for directive in essential_directives:
        if not re.search(directive, content):
            res.error(f"Missing essential directive matching: {directive}")

    # 4. Uninstaller Section Check
    if not re.search(r'Section\s+"Uninstall"', content):
        res.error('Missing Section "Uninstall"')

    # 5. URL Scheme & Uninstall Registry Keys
    if "HKCR" not in content or "infiltrator" not in content:
        res.error("Missing infiltrator:// URL protocol registration in HKCR")

    if "Uninstall\\Infiltrator" not in content:
        res.error("Missing Windows Add/Remove Programs registry key setup")

    return res


def validate_wix_package(wxs_path: pathlib.Path) -> ValidationResult:
    """Validates WiX installer XML configuration."""
    res = ValidationResult(str(wxs_path.relative_to(REPO_ROOT)))
    if not wxs_path.is_file():
        res.error("File does not exist")
        return res

    try:
        tree = ET.parse(wxs_path)
        root = tree.getroot()
        if not root.tag.endswith("Wix"):
            res.error(f"Root tag is not Wix (found {root.tag})")

        ns = {"wix": "http://wixtoolset.org/schemas/v4/wxs"}
        package = root.find("wix:Package", ns)
        if package is None:
            package = root.find("Package")
        if package is None:
            res.error("Missing <Package> element in WiX manifest")
        else:
            for attr in ["Name", "Manufacturer", "Version", "UpgradeCode"]:
                if attr not in package.attrib:
                    res.error(f"Missing required Package attribute: {attr}")

    except ET.ParseError as e:
        res.error(f"XML Parse Error in WiX file: {e}")
    except Exception as e:
        res.error(f"Error validating WiX file: {e}")

    return res


def validate_desktop_file(desktop_path: pathlib.Path) -> ValidationResult:
    """Validates Freedesktop .desktop entry specification compliance."""
    res = ValidationResult(str(desktop_path.relative_to(REPO_ROOT)))
    if not desktop_path.is_file():
        res.error("File does not exist")
        return res

    content = desktop_path.read_text(encoding="utf-8", errors="replace")
    lines = [line.strip() for line in content.splitlines() if line.strip() and not line.strip().startswith("#")]

    if not lines or lines[0] != "[Desktop Entry]":
        res.error("First non-comment entry must be [Desktop Entry]")

    fields = {}
    for line in lines[1:]:
        if "=" in line:
            k, v = line.split("=", 1)
            fields[k.strip()] = v.strip()

    mandatory = ["Type", "Name", "Exec", "Icon", "Categories"]
    for m in mandatory:
        if m not in fields:
            res.error(f"Missing mandatory desktop key: {m}")

    if fields.get("Type") != "Application":
        res.error(f"Invalid Type: expected Application, got {fields.get('Type')}")

    if "Categories" in fields and not fields["Categories"].endswith(";"):
        res.warning("Categories value should end with semicolon ';'")

    if "MimeType" in fields and not fields["MimeType"].endswith(";"):
        res.warning("MimeType value should end with semicolon ';'")

    return res


def validate_appimage_files(appimage_dir: pathlib.Path) -> list[ValidationResult]:
    """Validates Linux AppImage scripts and spec files."""
    results = []

    # 1. build-appimage.sh
    build_script = appimage_dir / "build-appimage.sh"
    res_build = ValidationResult(str(build_script.relative_to(REPO_ROOT)))
    for err in check_bash_syntax(build_script):
        res_build.error(err)
    if build_script.exists():
        content = build_script.read_text(encoding="utf-8", errors="replace")
        for expected in ["AppRun", "infiltrator.desktop", "appimagetool", "usr/bin", "usr/share/applications"]:
            if expected not in content:
                res_build.error(f"build-appimage.sh missing expected reference: {expected}")
    results.append(res_build)

    # 2. AppRun
    apprun_file = appimage_dir / "AppRun"
    res_apprun = ValidationResult(str(apprun_file.relative_to(REPO_ROOT)))
    for err in check_bash_syntax(apprun_file):
        res_apprun.error(err)
    if apprun_file.exists():
        content = apprun_file.read_text(encoding="utf-8", errors="replace")
        for env_var in ["APPDIR", "PATH", "LD_LIBRARY_PATH", "XDG_DATA_DIRS"]:
            if env_var not in content:
                res_apprun.error(f"AppRun missing environment setup for {env_var}")
        if "exec " not in content:
            res_apprun.error("AppRun missing exec invocation")
    results.append(res_apprun)

    # 3. .desktop
    desktop_file = appimage_dir / "infiltrator.desktop"
    results.append(validate_desktop_file(desktop_file))

    return results


def validate_debian_files(deb_dir: pathlib.Path) -> list[ValidationResult]:
    """Validates Debian packaging scripts and control files."""
    results = []

    # 1. build-deb.sh
    build_deb = deb_dir / "build-deb.sh"
    res_build = ValidationResult(str(build_deb.relative_to(REPO_ROOT)))
    for err in check_bash_syntax(build_deb):
        res_build.error(err)
    results.append(res_build)

    # 2. debian/control
    control_file = deb_dir / "debian" / "control"
    res_control = ValidationResult(str(control_file.relative_to(REPO_ROOT)))
    if not control_file.is_file():
        res_control.error("Missing debian/control file")
    else:
        content = control_file.read_text(encoding="utf-8", errors="replace")
        for field in ["Package:", "Version:", "Section:", "Priority:", "Architecture:", "Maintainer:", "Description:"]:
            if field not in content:
                res_control.error(f"debian/control missing required header: {field}")
    results.append(res_control)

    # 3. maintainer scripts (postinst, prerm, postrm)
    for script_name in ["postinst", "prerm", "postrm"]:
        script_file = deb_dir / "debian" / script_name
        if script_file.exists():
            res_script = ValidationResult(str(script_file.relative_to(REPO_ROOT)))
            for err in check_bash_syntax(script_file):
                res_script.error(err)
            results.append(res_script)

    return results


def validate_macos_files(macos_dir: pathlib.Path) -> list[ValidationResult]:
    """Validates macOS packaging scripts, entitlements, and Info.plist."""
    results = []

    # 1. create-dmg.sh
    create_dmg = macos_dir / "create-dmg.sh"
    res_dmg = ValidationResult(str(create_dmg.relative_to(REPO_ROOT)))
    for err in check_bash_syntax(create_dmg):
        res_dmg.error(err)
    if create_dmg.exists():
        content = create_dmg.read_text(encoding="utf-8", errors="replace")
        for expected in ["/Applications", "hdiutil", "create-dmg", "UDZO"]:
            if expected not in content:
                res_dmg.error(f"create-dmg.sh missing reference: {expected}")
    results.append(res_dmg)

    # 2. entitlements.plist
    entitlements_file = macos_dir / "entitlements.plist"
    res_ent = ValidationResult(str(entitlements_file.relative_to(REPO_ROOT)))
    if not entitlements_file.is_file():
        res_ent.error("Missing entitlements.plist")
    else:
        try:
            with open(entitlements_file, "rb") as f:
                data = plistlib.load(f)
            if not isinstance(data, dict):
                res_ent.error("entitlements.plist root must be a dictionary")
            else:
                expected_keys = [
                    "com.apple.security.cs.allow-jit",
                    "com.apple.security.network.client",
                    "com.apple.security.network.server",
                ]
                for k in expected_keys:
                    if k not in data:
                        res_ent.warning(f"entitlements.plist does not contain recommended key: {k}")
        except Exception as e:
            res_ent.error(f"Failed to parse entitlements.plist: {e}")
    results.append(res_ent)

    # 3. Info.plist
    info_file = macos_dir / "Info.plist"
    res_info = ValidationResult(str(info_file.relative_to(REPO_ROOT)))
    if not info_file.is_file():
        res_info.error("Missing Info.plist")
    else:
        try:
            with open(info_file, "rb") as f:
                data = plistlib.load(f)
            if not isinstance(data, dict):
                res_info.error("Info.plist root must be a dictionary")
            else:
                required_bundle_keys = [
                    "CFBundleExecutable",
                    "CFBundleIdentifier",
                    "CFBundleName",
                    "CFBundlePackageType",
                    "CFBundleShortVersionString",
                    "CFBundleVersion",
                ]
                for k in required_bundle_keys:
                    if k not in data:
                        res_info.error(f"Info.plist missing required bundle key: {k}")
        except Exception as e:
            res_info.error(f"Failed to parse Info.plist: {e}")
    results.append(res_info)

    return results


def validate_release_manifest_generator(manifest_script: pathlib.Path) -> ValidationResult:
    """Validates generate-release-manifest.py execution and catalog generation."""
    res = ValidationResult(str(manifest_script.relative_to(REPO_ROOT)))
    if not manifest_script.is_file():
        res.error("Missing generate-release-manifest.py")
        return res

    # 1. Check Python syntax
    try:
        proc = subprocess.run(
            [sys.executable, "-m", "py_compile", str(manifest_script)],
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode != 0:
            res.error(f"Python compilation error: {proc.stderr.strip()}")
            return res
    except Exception as e:
        res.error(f"Failed to compile Python script: {e}")
        return res

    # 2. Functional Test in Temporary Directory
    tmp_root = os.environ.get("TMPDIR", "/tmp")
    with tempfile.TemporaryDirectory(dir=tmp_root) as tmp_dir_str:
        tmp_dir = pathlib.Path(tmp_dir_str)
        dist_mock = tmp_dir / "dist"
        dist_mock.mkdir()

        # Create mock build artifacts with realistic multi-format targets
        sample_artifacts = [
            "Infiltrator-Setup-x86_64.exe",
            "Infiltrator-Windows-x86_64.zip",
            "Infiltrator-macOS-arm64.dmg",
            "Infiltrator-macOS-arm64.tar.gz",
            "Infiltrator-Linux-x86_64.AppImage",
            "infiltrator_0.20.0_amd64.deb",
            "Infiltrator-Linux-x86_64.tar.gz",
            "infiltrator-arm64-v8a-release.apk",
        ]
        for name in sample_artifacts:
            (dist_mock / name).write_bytes(f"binary content for {name}\n".encode("utf-8"))

        manifest_out = tmp_dir / "release-manifest.json"
        latest_out = tmp_dir / "latest.json"
        sha256_out = tmp_dir / "SHA256SUMS.txt"

        try:
            cmd = [
                sys.executable,
                str(manifest_script),
                "--version", "0.20.0",
                "--channel", "stable",
                "--dist-dir", str(dist_mock),
                "--output-manifest", str(manifest_out),
                "--output-latest", str(latest_out),
                "--output-sha256sums", str(sha256_out),
            ]
            run_res = subprocess.run(cmd, capture_output=True, text=True, check=False)
            if run_res.returncode != 0:
                res.error(f"generate-release-manifest.py execution failed: {run_res.stderr}")
                return res

            if not manifest_out.exists() or not latest_out.exists() or not sha256_out.exists():
                res.error("generate-release-manifest.py did not produce all expected output files")
                return res

            # Verify sha256sums format
            sha_lines = sha256_out.read_text(encoding="utf-8").strip().splitlines()
            if len(sha_lines) != len(sample_artifacts):
                res.error(f"SHA256SUMS count mismatch: expected {len(sample_artifacts)}, got {len(sha_lines)}")

            for line in sha_lines:
                parts = line.split(maxsplit=1)
                if len(parts) != 2 or len(parts[0]) != 64:
                    res.error(f"Invalid sha256 line format: {line}")

            # Verify JSON manifest and latest payload structures
            import json
            manifest_data = json.loads(manifest_out.read_text(encoding="utf-8"))
            latest_data = json.loads(latest_out.read_text(encoding="utf-8"))

            m_artifacts = manifest_data.get("artifacts", [])
            if len(m_artifacts) != len(sample_artifacts):
                res.error(f"release-manifest.json artifacts count mismatch: expected {len(sample_artifacts)}, got {len(m_artifacts)}")

            l_artifacts = latest_data.get("artifacts", {})
            if not isinstance(l_artifacts, dict) or not l_artifacts:
                res.error("latest.json 'artifacts' must be a non-empty dictionary for legacy compatibility")

            l_packages = latest_data.get("packages", [])
            if not isinstance(l_packages, list) or len(l_packages) != len(sample_artifacts):
                res.error(f"latest.json 'packages' count mismatch: expected {len(sample_artifacts)}, got {len(l_packages)}")

            pkg_names = {p.get("name") for p in l_packages}
            expected_names = set(sample_artifacts)
            if pkg_names != expected_names:
                res.error(f"latest.json packages names mismatch: diff={expected_names ^ pkg_names}")

            for pkg in l_packages:
                if not pkg.get("target_triple") or not pkg.get("sha256") or not pkg.get("url") or pkg.get("size", 0) <= 0:
                    res.error(f"Invalid package metadata record in latest.json: {pkg}")

        except Exception as e:
            res.error(f"Functional manifest test exception: {e}")

    return res


def validate_signing_scripts(scripts_dir: pathlib.Path) -> list[ValidationResult]:
    """Validates helper signing scripts in scripts/."""
    results = []

    for name in ["sign-macos.sh", "sign-windows.sh"]:
        path = scripts_dir / name
        if path.exists():
            res = ValidationResult(str(path.relative_to(REPO_ROOT)))
            for err in check_bash_syntax(path):
                res.error(err)
            results.append(res)

    ps1_path = scripts_dir / "sign-windows.ps1"
    if ps1_path.exists():
        res_ps1 = ValidationResult(str(ps1_path.relative_to(REPO_ROOT)))
        content = ps1_path.read_text(encoding="utf-8", errors="replace")
        for kw in ["param", "signtool.exe", "Authenticode", "TimestampServers"]:
            if kw not in content:
                res_ps1.error(f"sign-windows.ps1 missing essential keyword: {kw}")
        results.append(res_ps1)

    return results


def main() -> int:
    parser = argparse.ArgumentParser(description="Packaging Template & Release Manifest Guard")
    parser.add_argument("--verbose", "-v", action="store_true", help="Show details for passed checks")
    args = parser.parse_args()

    results: list[ValidationResult] = []

    # 1. Windows NSIS & WiX
    results.append(validate_nsis_script(REPO_ROOT / "packaging/windows/nsis/infiltrator.nsi"))
    results.append(validate_wix_package(REPO_ROOT / "packaging/windows/wix/Package.wxs"))

    # 2. Linux AppImage
    results.extend(validate_appimage_files(REPO_ROOT / "packaging/linux/appimage"))

    # 3. Linux Debian
    results.extend(validate_debian_files(REPO_ROOT / "packaging/linux/deb"))

    # 4. macOS Packaging
    results.extend(validate_macos_files(REPO_ROOT / "packaging/macos"))

    # 5. Release Manifest Generator Tool
    results.append(validate_release_manifest_generator(REPO_ROOT / "scripts/generate-release-manifest.py"))

    # 6. Signing Automation Scripts
    results.extend(validate_signing_scripts(REPO_ROOT / "scripts"))

    # Summary
    failed_results = [r for r in results if not r.is_ok]
    total_checks = len(results)
    passed_checks = total_checks - len(failed_results)

    print(f"\n========================================================")
    print(f" Packaging & Release Asset Validation Summary")
    print(f" Total targets inspected: {total_checks}")
    print(f" Passed: {passed_checks} | Failed: {len(failed_results)}")
    print(f"========================================================")

    for r in results:
        status_tag = "[PASS]" if r.is_ok else "[FAIL]"
        if not r.is_ok or args.verbose:
            print(f"{status_tag} {r.target}")
            for err in r.errors:
                print(f"    ERROR: {err}")
            for warn in r.warnings:
                print(f"    WARN : {warn}")

    if failed_results:
        print("\nPackaging verification FAILED.")
        return 1

    print("\nAll packaging templates, scripts, plists, and manifests are VALID.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
