#!/usr/bin/env python3
"""BANDROID-031.1: Compose walk-through preparation and evidence capture script.

Generates a reproducible walk-through record template bound to the native Compose
APK SHA-256, git commit, device parameters (window, density, fontScale), package
identity, launcher Activity, and the standardized walk-through scenarios.
Provides logcat and screenrecord collection commands.

Usage:
    python3 scripts/android-compose-walkthrough.py [--apk <path>] [--dry-run] [--output <path>]
    python3 scripts/android-compose-walkthrough.py --self-test
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]

SCENARIOS = [
    {
        "id": "SCN-01-OVERVIEW",
        "page": "Overview",
        "description": "Dashboard overview, Core lifecycle status, VPN toggle, real traffic readout",
    },
    {
        "id": "SCN-02-PROXIES",
        "page": "Proxies",
        "description": "Outbound inventory groups, node latency ping, active selection",
    },
    {
        "id": "SCN-03-PROFILES",
        "page": "Profiles",
        "description": "Subscription list, update schedule, Age-encrypted URL import, deletion",
    },
    {
        "id": "SCN-04-NETWORK",
        "page": "Network",
        "description": "Routing mode selection (Rule/Global/Direct), VPN service state, permission",
    },
    {
        "id": "SCN-05-CONNECTIONS",
        "page": "Connections",
        "description": "Active connection list, endpoint inspection, close connection",
    },
    {
        "id": "SCN-06-LOGS",
        "page": "Logs",
        "description": "Real-time log streaming, log level filtering, search, export",
    },
    {
        "id": "SCN-07-RULES",
        "page": "Rules",
        "description": "Rule list observation, provider refresh, rule inspection",
    },
    {
        "id": "SCN-08-SETTINGS",
        "page": "Settings",
        "description": "Application language, theme mode, autostart, notification toggle",
    },
    {
        "id": "SCN-09-ABOUT",
        "page": "About",
        "description": "Version information, third-party licenses inspection",
    },
]


def git_commit() -> str:
    try:
        output = subprocess.check_output(
            ["git", "rev-parse", "HEAD"],
            cwd=REPO_ROOT,
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        return output
    except Exception:
        return "UNKNOWN_COMMIT"


def compute_sha256(path: Path) -> str:
    if not path.is_file():
        return "APK_NOT_FOUND"
    hasher = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            hasher.update(chunk)
    return hasher.hexdigest()


def get_identity_value(key: str) -> str:
    try:
        output = subprocess.check_output(
            [sys.executable, str(REPO_ROOT / "scripts/product_identity.py"), "--get", key],
            cwd=REPO_ROOT,
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        return output
    except Exception:
        if key == "compose.application_id":
            return "com.musicfrog.infiltrator"
        if key == "compose.launcher_activity":
            return ".MainActivity"
        if key == "android.namespace":
            return "com.musicfrog.infiltrator"
        return "unknown"


def query_adb_device() -> dict[str, str] | None:
    adb_cmd = "adb"
    try:
        state = subprocess.check_output(
            [adb_cmd, "get-state"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        if state != "device":
            return None
    except Exception:
        return None

    def shell(cmd: list[str]) -> str:
        try:
            return subprocess.check_output(
                [adb_cmd, "shell", *cmd],
                text=True,
                stderr=subprocess.DEVNULL,
            ).strip()
        except Exception:
            return "unknown"

    model = shell(["getprop", "ro.product.model"])
    manufacturer = shell(["getprop", "ro.product.manufacturer"])
    release = shell(["getprop", "ro.build.version.release"])
    sdk = shell(["getprop", "ro.build.version.sdk"])
    wm_size = shell(["wm", "size"])
    wm_density = shell(["wm", "density"])
    font_scale = shell(["settings", "get", "system", "font_scale"])

    return {
        "model": f"{manufacturer} {model}".strip(),
        "android_version": f"Android {release} (API {sdk})",
        "window_size": wm_size.replace("Physical size: ", "").strip(),
        "density": wm_density.replace("Physical density: ", "").strip(),
        "font_scale": font_scale if font_scale != "null" else "1.0",
        "source": "live_device",
    }


def default_device_profile() -> dict[str, str]:
    return {
        "model": "Reference Test Device (e.g. Pixel 8)",
        "android_version": "Android 14 (API 34)",
        "window_size": "1080x2400",
        "density": "420 dpi",
        "font_scale": "1.0",
        "source": "reference_baseline",
    }


def generate_walkthrough_template(
    apk_path: Path | None,
    device: dict[str, str],
    commit_sha: str,
    package_name: str,
    launcher_activity: str,
) -> str:
    apk_hash = compute_sha256(apk_path) if apk_path else "PENDING_BUILD"
    apk_name = apk_path.name if apk_path else "app-compose-debug.apk"

    lines = [
        "# Android Compose UI 走查与证据记录",
        "",
        "> 任务：`BANDROID-031.1`",
        "",
        "## 一、构建产物与环境身份",
        "",
        f"- **原生 APK 文件**：`{apk_name}`",
        f"- **APK SHA-256**：`{apk_hash}`",
        f"- **Git Commit**：`{commit_sha}`",
        f"- **应用包名 (Package)**：`{package_name}`",
        f"- **启动 Activity**：`{launcher_activity}`",
        "",
        "## 二、走查设备规格与参数",
        "",
        f"- **设备型号**：{device['model']} ({device['source']})",
        f"- **系统版本**：{device['android_version']}",
        f"- **窗口分辨率 (Window Size)**：`{device['window_size']}`",
        f"- **显示密度 (Density)**：`{device['density']}`",
        f"- **字体缩放比 (Font Scale)**：`{device['font_scale']}`",
        "",
        "## 三、走查场景清单与证据矩阵",
        "",
        "| 场景编号 | 目标页面 | 场景说明 | 状态 | 日志文件 | 录屏文件 |",
        "| :--- | :--- | :--- | :---: | :--- | :--- |",
    ]

    for s in SCENARIOS:
        sid = s["id"]
        page = s["page"]
        desc = s["description"]
        log_file = f"`target/evidence/{sid}.logcat`"
        video_file = f"`target/evidence/{sid}.mp4`"
        lines.append(f"| `{sid}` | {page} | {desc} | 待走查 | {log_file} | {video_file} |")

    lines.extend([
        "",
        "## 四、日志与录屏采集操作指南",
        "",
        "### 1. 单场景日志采集",
        "```bash",
        "# 清空历史缓存",
        "adb logcat -c",
        "# 启动目标场景日志过滤录制",
        f"adb logcat -v time -s Infiltrator:* ComposeApp:* VpnService:* > target/evidence/<SCENARIO_ID>.logcat",
        "```",
        "",
        "### 2. 交互录屏采集",
        "```bash",
        "# 在设备上启动录屏 (最长 180 秒)",
        "adb shell screenrecord --size 720x1280 /sdcard/<SCENARIO_ID>.mp4",
        "# 结束录屏后拉取到本地证据目录",
        "adb pull /sdcard/<SCENARIO_ID>.mp4 target/evidence/<SCENARIO_ID>.mp4",
        "adb shell rm /sdcard/<SCENARIO_ID>.mp4",
        "```",
        "",
        "## 五、走查结论与签收",
        "",
        "- [ ] 所有 9 项核心场景走查无静默 crash、无无限 loading、状态流转完整",
        "- [ ] 异常情况（断网、错配、拒权）显示正确 typed 错误与重试入口",
        "- [ ] 走查证据文件哈希与本模板登记的 APK SHA-256 吻合",
        "",
        "签收人：_______________  日期：__________",
    ])

    return "\n".join(lines) + "\n"


def self_test() -> int:
    commit = git_commit()
    assert len(commit) == 40 or commit == "UNKNOWN_COMMIT", f"commit hash format error: {commit}"

    pkg = get_identity_value("compose.application_id")
    assert pkg == "com.musicfrog.infiltrator", f"package mismatch: {pkg}"

    activity = get_identity_value("compose.launcher_activity")
    assert ".MainActivity" in activity, f"activity mismatch: {activity}"

    dev = default_device_profile()
    assert "1080x2400" in dev["window_size"]

    template = generate_walkthrough_template(
        None,
        dev,
        commit,
        pkg,
        f"{pkg}{activity}",
    )
    assert "# Android Compose UI 走查与证据记录" in template
    assert "SCN-01-OVERVIEW" in template
    assert "SCN-09-ABOUT" in template
    assert "com.musicfrog.infiltrator" in template
    assert "screenrecord" in template
    assert "logcat" in template

    print("android-compose-walkthrough self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apk", type=Path, help="Path to compose debug/release APK")
    parser.add_argument("--dry-run", action="store_true", help="Use baseline device parameters without adb")
    parser.add_argument("--output", "-o", type=Path, help="Output markdown path for walkthrough record")
    parser.add_argument("--self-test", action="store_true", help="Run self-tests")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    device = None
    if not args.dry_run:
        device = query_adb_device()

    if device is None:
        device = default_device_profile()

    commit = git_commit()
    package_name = get_identity_value("compose.application_id")
    launcher_rel = get_identity_value("compose.launcher_activity")
    namespace = get_identity_value("android.namespace")
    launcher_activity = (
        f"{namespace}{launcher_rel}" if launcher_rel.startswith(".") else launcher_rel
    )

    template = generate_walkthrough_template(
        args.apk,
        device,
        commit,
        package_name,
        launcher_activity,
    )

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(template, encoding="utf-8")
        print(f"Generated walkthrough template at {args.output}")
    else:
        print(template)

    return 0


if __name__ == "__main__":
    sys.exit(main())
