"""Drive the Android interaction flow against a fake adb device.

The fake device answers the same adb command line the flow uses and keeps a
small screen state: a launch, a navigation, a scroll, a consent dialog and a
notification dialog.  The test proves three things that matter for the
evidence: the flow produces receipts, an action that cannot be resolved
produces no receipt, and a screen that does not change produces no receipt
either.
"""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
ANDROID = ROOT / "scripts/android"

# The flow modules import each other by name, so the directory has to be on
# the path before they are loaded.
sys.path.insert(0, str(ANDROID))
sys.path.insert(0, str(ROOT / "scripts"))


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


for _module in ("capture_actions", "capture_anchors", "capture_device", "capture_evidence",
                "capture_hierarchy", "capture_pixels", "capture_scenarios"):
    _load(_module, ANDROID / f"{_module}.py")

capture_flow = _load("capture_flow", ANDROID / "capture_flow.py")
capture_evidence = sys.modules["capture_evidence"]
capture_scenarios = sys.modules["capture_scenarios"]
capture_anchors = sys.modules["capture_anchors"]

try:
    from PIL import Image

    HAS_PIL = True
except ImportError:  # pragma: no cover - the desktop capture needs Pillow too
    HAS_PIL = False


FAKE_ADB = '''#!/usr/bin/env python3
"""A fake adb that answers the flow's command line."""
import json
import os
import pathlib
import sys

STATE = pathlib.Path(os.environ["FAKE_ADB_STATE"])
PACKAGE = "com.musicfrog.infiltrator"


def load():
    return json.loads(STATE.read_text())


def save(state):
    STATE.write_text(json.dumps(state))


def png(state):
    import io

    from PIL import Image

    image = Image.new("RGB", (state["width"], state["height"]), tuple(state["color"]))
    buffer = io.BytesIO()
    image.save(buffer, "PNG")
    sys.stdout.buffer.write(buffer.getvalue())


def hierarchy(state):
    nodes = []
    for index, (text, top) in enumerate(state["nodes"]):
        left, right = 40, state["width"] - 40
        nodes.append(
            '<node class="android.widget.TextView" text="%s" content-desc="" '
            'resource-id="" clickable="true" bounds="[%d,%d][%d,%d]" />'
            % (text, left, top, right, top + 120)
        )
    return (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<hierarchy rotation="0">'
        + "".join(nodes)
        + "</hierarchy>"
    )


def main(argv):
    state = load()
    if "devices" in argv:
        print("List of devices attached")
        print("emulator-5554\\tdevice product:emu model:Pixel_6 device:emu64a transport_id:1")
        return 0
    arguments = [value for value in argv if value not in ("-s", "emulator-5554")]
    if arguments[:1] == ["exec-out"]:
        command = arguments[1:]
        if command[:1] == ["screencap"]:
            png(state)
            return 0
        if command[:1] == ["uiautomator"]:
            print(hierarchy(state))
            return 0
        if command[:1] == ["logcat"]:
            for line in state["logcat"]:
                print(line)
            return 0
    if arguments[:1] == ["shell"]:
        command = arguments[1:]
        if command[:2] == ["wm", "size"]:
            print("Physical size: %dx%d" % (state["width"], state["height"]))
            return 0
        if command[:2] == ["wm", "density"]:
            print("Physical density: 420")
            return 0
        if command[:1] == ["pm"] and command[1:2] == ["install"]:
            print("Success")
            return 0
        if command[:2] == ["settings", "get"]:
            print("1")
            return 0
        if command[:1] == ["am"]:
            if command[1:2] == ["start"]:
                state["color"] = [0, 0, 0]
                state["nodes"] = [["Overview", 100], ["Profiles", 2100], ["Proxies", 2100], ["Settings", 2100]]
                state["logcat"].append("I/MihomoApp  ( 1234): Initializing application...")
                state["logcat"].append("I/ActivityTaskManager: Displayed com.musicfrog.infiltrator/.MainActivity: +120ms")
                save(state)
                print("Status: ok")
                return 0
        if command[:1] == ["input"]:
            if command[1:2] == ["tap"]:
                x, y = int(command[2]), int(command[3])
                label = ""
                for text, top in state["nodes"]:
                    if top <= y <= top + 120:
                        label = text
                        break
                if label == "Start":
                    state["color"] = [60, 60, 60]
                    state["nodes"] = [["Connection request", 900], ["OK", 1500], ["Cancel", 1700]]
                    state["logcat"].append("I/ActivityTaskManager: START u0 {cmp=com.android.vpndialogs/.ConfirmDialog}")
                elif label == "OK":
                    state["color"] = [120, 120, 120]
                    state["nodes"] = [["Allow MusicFrog Infiltrator to send notifications?", 900], ["Allow", 1500], ["Do not allow", 1700]]
                    state["logcat"].append("I/PermissionController: Granting notification permission")
                elif label == "Allow":
                    state["color"] = [180, 180, 180]
                    state["nodes"] = [["Overview", 100], ["Start", 400], ["Profiles", 2100]]
                    state["logcat"].append("I/MihomoVpnService: VPN cleanup reached owner terminal state: Stopped")
                elif label == "Profiles":
                    state["color"] = [200, 200, 200]
                    state["nodes"] = [["Profiles", 100], ["Overview", 2100], ["Proxies", 2100], ["Settings", 2100]]
                elif label == "Overview":
                    state["color"] = [0, 0, 0]
                    state["nodes"] = [["Overview", 100], ["Start", 400], ["Profiles", 2100], ["Proxies", 2100], ["Settings", 2100]]
                save(state)
                return 0
            if command[1:2] == ["swipe"]:
                state["color"] = [240, 240, 240]
                state["nodes"] = [["Overview", 100], ["Overview", 300], ["Start", 500], ["Profiles", 2100]]
                state["logcat"].append("I/MihomoVpnService: VPN cleanup reached owner terminal state: Stopped")
                save(state)
                return 0
        if command[:1] == ["dumpsys"]:
            if command[1:2] == ["activity"]:
                print("  mResumedActivity: ActivityRecord{a1 u0 %s/.MainActivity t42}" % PACKAGE)
                return 0
            if command[1:2] == ["window"]:
                print("  mCurrentFocus=Window{abc123 u0 %s/%s.MainActivity}" % (PACKAGE, PACKAGE))
                return 0
            if command[1:2] == ["package"]:
                print("    permission android.permission.POST_NOTIFICATIONS: granted=true")
                return 0
            if command[1:2] == ["connectivity"]:
                print("    VPN: TRANSPORT_VPN %s" % PACKAGE)
                return 0
        if command[:1] == ["pidof"]:
            print("1234")
            return 0
        if command[:1] == ["am"] or command[:1] == ["pm"]:
            return 0
        print("")
        return 0
    print("")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
'''


@unittest.skipUnless(HAS_PIL, "Pillow is required for the fake device screenshots")
class AndroidInteractionFlowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="android-interaction-flow-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        # The evidence has to live inside the repository: the receipt paths are
        # recorded relative to the repository root.
        self.output = ROOT / "target/android-interaction-flow-test" / self.directory.name
        self.addCleanup(shutil.rmtree, self.output, True)
        self.state = self.directory / "state.json"
        self.state.write_text(
            json.dumps(
                {
                    "width": 1080,
                    "height": 2400,
                    "color": [250, 250, 250],
                    "nodes": [],
                    "logcat": [],
                }
            )
        )
        self.adb = self.directory / "fake-adb"
        self.adb.write_text(FAKE_ADB)
        self.adb.chmod(self.adb.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
        self.apk = self.output.parent / "candidate.apk"
        # The evidence directory lives inside the repository, so its parent has
        # to exist before the candidate is written.
        self.apk.parent.mkdir(parents=True, exist_ok=True)
        self.apk.write_bytes(b"not a real apk, the fake device accepts it")
        os.environ["FAKE_ADB_STATE"] = str(self.state)
        self.addCleanup(os.environ.pop, "FAKE_ADB_STATE", None)

    def _flow(self, scenarios_path: Path) -> capture_flow.CaptureFlow:
        scenarios = capture_scenarios.load_scenarios(scenarios_path)
        anchors = capture_anchors.load_anchors(ANDROID / "android_interaction_anchors.tsv")
        options = capture_flow.FlowOptions(
            flavor="compose",
            apk=self.apk,
            output=self.output,
            adb=self.adb,
            serial="emulator-5554",
            settle_seconds=0.0,
            dialog_timeout_seconds=2.0,
        )
        return capture_flow.CaptureFlow(options, scenarios, anchors)

    def test_the_flow_writes_a_receipt_for_every_proven_step(self) -> None:
        flow = self._flow(ANDROID / "android_interaction_scenarios.tsv")
        receipt = flow.run()
        proven = {step.step_id for step in receipt.steps}
        self.assertIn("startup", proven)
        self.assertIn("navigation", proven)
        self.assertIn("list-scroll", proven)
        for step in receipt.steps:
            self.assertTrue(step.business_id)
            self.assertTrue(step.logcat.sha256)
            self.assertTrue(step.frame.sha256)
            if "pixel-delta" in [kind.value for kind in step.evidence]:
                self.assertTrue(step.pixel_delta["corroborated"])
        self.assertEqual(receipt.unproven, ())

    def test_the_receipts_the_flow_writes_pass_the_guard(self) -> None:
        guard = _load(
            "android_interaction_receipt_guard",
            ROOT / "scripts/quality/android-interaction-receipt-guard.py",
        )
        flow = self._flow(ANDROID / "android_interaction_scenarios.tsv")
        receipt = flow.run()
        document = capture_evidence.build_document(ROOT, [receipt], {"git_head": "test"})
        self.assertEqual(guard.receipt_problems(ROOT, document), [])

    def test_a_step_without_an_evidence_produces_no_receipt(self) -> None:
        table = ANDROID / "android_interaction_scenarios.tsv"
        rows = table.read_text(encoding="utf-8").splitlines()
        mutated: list[str] = []
        for row in rows:
            if row.startswith("navigation\t"):
                fields = row.split("\t")
                fields[3] = "tap_text:ThereIsNoSuchLabel"
                row = "\t".join(fields)
            mutated.append(row)
        broken = self.directory / "broken-scenarios.tsv"
        broken.write_text("\n".join(mutated) + "\n", encoding="utf-8")
        flow = self._flow(broken)
        receipt = flow.run()
        proven = {step.step_id for step in receipt.steps}
        self.assertNotIn("navigation", proven)
        outcomes = {record["step_id"]: record["outcome"] for record in receipt.unproven}
        self.assertEqual(outcomes.get("navigation"), "no_observable_change")
        # The dependent steps are blocked, not silently reported as proven.
        self.assertEqual(outcomes.get("list-scroll"), "blocked")


if __name__ == "__main__":
    unittest.main()
