"""The Android interaction capture flow.

One flavor, one APK, one emulator.  Every declared step is driven on the
device and then judged against the evidence the scenario table demands:

* the screenshot has to differ from the previous state by more than the
  surface's own noise floor — a static screen is never an interaction;
* the logcat window of the step has to carry the step's marker;
* the accessibility assertion has to hold;
* the owner state readback has to produce a value.

A step that fails any of these becomes an *unproven* record, never a receipt.
"""

from __future__ import annotations

import dataclasses
import datetime
import hashlib
import pathlib
import re
import subprocess
import sys
import time

from capture_actions import Action, ActionKind
from capture_anchors import AnchorTable, NavAnchor
from capture_device import (
    Adb,
    AdbError,
    DeviceIdentity,
    list_devices,
    select_emulator_device,
)
from capture_evidence import (
    FlavorReceipt,
    Frame,
    LogcatEvidence,
    StepOutcome,
    StepReceipt,
)
from capture_hierarchy import HierarchyNode, Region, parse_hierarchy, select_node
from capture_pixels import compare_frames, png_dimensions, sha256_file
from capture_scenarios import (
    EvidenceKind,
    Scenario,
    StateReadbackKind,
)

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import product_identity  # noqa: E402  (the product identity registry is the only source)

# The permission the notification prompt asks for.
NOTIFICATION_PERMISSION = "android.permission.POST_NOTIFICATIONS"

# The fraction of the screen height the bottom navigation strip occupies.  A
# navigation tap is judged on the content above it, never on the strip itself.
NAVIGATION_BAND = 0.10


class FlowError(RuntimeError):
    """The flow cannot run at all."""


@dataclasses.dataclass(frozen=True)
class FlowOptions:
    """Everything the operator can decide about one run."""

    flavor: str
    apk: pathlib.Path
    output: pathlib.Path
    adb: pathlib.Path
    serial: str | None = None
    settle_seconds: float = 1.5
    dialog_timeout_seconds: float = 25.0
    keep_installed: bool = False


class LogcatWindow:
    """The logcat of the capture session.

    ``read`` returns the lines the device produced since the last read and
    keeps them in the session log: a step's marker can be produced a moment
    before the step that has to answer it (the system raises the notification
    permission dialog right after the consent dialog returns), so the marker is
    looked up in the session, not only in the step's own lines.
    """

    def __init__(self, adb: Adb) -> None:
        self.adb = adb
        self.cursor = 0
        self.session: list[str] = []

    def read(self) -> tuple[str, ...]:
        lines = self.adb.logcat().splitlines()
        if self.cursor > len(lines):
            # The ring buffer wrapped around; the window starts over.
            self.cursor = 0
        fresh = lines[self.cursor :]
        self.cursor = len(lines)
        self.session.extend(fresh)
        return tuple(fresh)

    def session_lines(self) -> tuple[str, ...]:
        return tuple(self.session)


class CaptureFlow:
    """Drives one flavor of the product on one emulator."""

    def __init__(self, options: FlowOptions, scenarios, anchors: AnchorTable) -> None:
        self.options = options
        self.scenarios = scenarios
        self.anchors = anchors
        self.device: DeviceIdentity | None = None
        self.adb: Adb | None = None
        self.logcat: LogcatWindow | None = None
        self.screen: tuple[int, int] = (0, 0)
        self.package = ""
        self.component = ""
        self.run_directory = options.output / options.flavor

    # -- identity ------------------------------------------------------------

    def _identity(self, key: str) -> str:
        values = product_identity.read_properties()
        return product_identity.get_value(values, key)

    def _component(self) -> str:
        """The launcher component of the flavor, from the identity registry."""
        activity = self._identity(f"{self.options.flavor}.launcher_activity")
        return f"{self.package}/{activity}"

    # -- setup ---------------------------------------------------------------

    def prepare(self) -> None:
        """Select the emulator, install the APK and pin the guest offline."""
        if self.options.serial:
            devices = [DeviceIdentity(serial=self.options.serial, state="device")]
        else:
            devices = list_devices(self.options.adb)
        self.device = select_emulator_device(devices)
        self.adb = Adb(self.options.adb, self.device.serial)
        self.device = self.adb.describe_device(self.device)
        self.screen = self.adb.screen_size()
        self.package = self._identity(f"{self.options.flavor}.application_id")
        self.component = self._component()
        self.logcat = LogcatWindow(self.adb)

        if not self.options.apk.is_file():
            raise FlowError(f"the APK does not exist: {self.options.apk}")
        # The captured APK is always the candidate: an older build of the same
        # product must never stand in for it.
        report = self.adb.install(self.options.apk)
        if "Success" not in report:
            raise FlowError(
                f"the {self.options.flavor} APK could not be installed: {report.strip()}"
            )
        self.adb.force_stop(self.package)
        # The two system prompts have to be real: the product starts the
        # capture without a VPN consent and without the notification
        # permission, so both dialogs really have to be answered.
        self.adb.shell("pm", "revoke", self.package, NOTIFICATION_PERMISSION)
        self.adb.shell("appops", "set", self.package, "ACTIVATE_VPN", "ignore")
        # The guest stays offline: the captured product must not reach the
        # network while the evidence is collected.
        if not self.adb.airplane_mode():
            self.adb.set_airplane_mode(True)
            time.sleep(2.0)
        if not self.adb.airplane_mode():
            raise FlowError("the emulator guest could not be put into airplane mode")

    # -- frames --------------------------------------------------------------

    def _capture(self, directory: pathlib.Path, name: str) -> Frame:
        assert self.adb is not None
        data = self.adb.screenshot_png()
        if not data.startswith(b"\x89PNG\r\n\x1a\n"):
            raise FlowError("the device did not return a PNG screenshot")
        path = directory / f"{name}.png"
        path.write_bytes(data)
        width, height = png_dimensions(path)
        return Frame(
            name=name,
            path=path,
            sha256=sha256_file(path),
            width=width,
            height=height,
        )

    def _content_bounds(self) -> tuple[float, float, float, float]:
        """The region above the navigation strip."""
        width, height = self.screen
        return (0.0, 0.0, float(width), float(height - round(height * NAVIGATION_BAND)))

    # -- actions -------------------------------------------------------------

    def _hierarchy(self) -> list[HierarchyNode]:
        assert self.adb is not None
        return parse_hierarchy(self.adb.ui_hierarchy())

    def _wait_for_node(self, labels: tuple[str, ...]) -> HierarchyNode | None:
        assert self.adb is not None
        deadline = time.monotonic() + self.options.dialog_timeout_seconds
        while True:
            try:
                node = select_node(self._hierarchy(), labels, Region.ANY, self.screen)
            except (AdbError, ValueError):
                node = None
            if node is not None:
                return node
            if time.monotonic() >= deadline:
                return None
            time.sleep(0.6)

    def _perform(
        self,
        action: Action,
        directory: pathlib.Path,
        before: Frame,
    ) -> dict[str, object]:
        """Run one declared action and describe how it was resolved."""
        assert self.adb is not None
        record: dict[str, object] = {
            "kind": action.kind.value,
            "argument": action.argument,
            "raw": action.raw,
        }
        if action.kind is ActionKind.LAUNCH:
            output = self.adb.launch(self.component)
            record["resolved_by"] = "activity"
            record["detail"] = {"output": output.strip()}
            return record

        if action.kind is ActionKind.TAP_TEXT:
            try:
                node = select_node(
                    self._hierarchy(), action.labels(), Region.ANY, self.screen
                )
            except (AdbError, ValueError):
                node = None
            if node is None:
                record["resolved_by"] = "unresolved"
                record["detail"] = {"reason": "no node carries the requested label"}
                return record
            self.adb.tap(*node.center)
            record["resolved_by"] = "accessibility"
            record["detail"] = node.as_record()
            return record

        if action.kind is ActionKind.WAIT_TEXT:
            node = self._wait_for_node(action.labels())
            if node is None:
                record["resolved_by"] = "unresolved"
                record["detail"] = {"reason": "the requested label never appeared"}
                return record
            record["resolved_by"] = "accessibility"
            record["detail"] = node.as_record()
            return record

        if action.kind is ActionKind.TAP_DIALOG:
            node = self._wait_for_node(action.labels())
            if node is None:
                record["resolved_by"] = "unresolved"
                record["detail"] = {"reason": "the dialog never appeared"}
                return record
            self.adb.tap(*node.center)
            record["resolved_by"] = "accessibility"
            record["detail"] = node.as_record()
            return record

        if action.kind is ActionKind.REVOKE_PERMISSION:
            self.adb.revoke_permission(self.package, action.argument)
            record["resolved_by"] = "package-manager"
            record["detail"] = {"permission": action.argument}
            return record

        if action.kind in (ActionKind.SWIPE_UP, ActionKind.SWIPE_DOWN):
            width, height = self.screen
            if action.kind is ActionKind.SWIPE_UP:
                start, end = round(height * 0.70), round(height * 0.25)
            else:
                start, end = round(height * 0.30), round(height * 0.75)
            self.adb.swipe(width // 2, start, width // 2, end, 400)
            record["resolved_by"] = "input"
            record["detail"] = {"from": [width // 2, start], "to": [width // 2, end]}
            return record

        if action.kind in (ActionKind.TAP_NAV, ActionKind.TAP_ANCHOR):
            return self._perform_anchor(action, directory, before)

        raise FlowError(f"the action {action.kind.value!r} is not implemented")

    def _perform_anchor(
        self,
        action: Action,
        directory: pathlib.Path,
        before: Frame,
    ) -> dict[str, object]:
        """Tap a calibrated control of a native surface.

        A native surface publishes no accessibility tree, so the tap point
        comes from the calibrated anchor.  The candidates are tried from the
        most likely one; a candidate counts only when the content above the
        navigation strip really changed.
        """
        assert self.adb is not None
        record: dict[str, object] = {
            "kind": action.kind.value,
            "argument": action.argument,
            "raw": action.raw,
        }
        anchor = self.anchors.for_name(self.options.flavor, action.argument)
        if anchor is None:
            record["resolved_by"] = "unresolved"
            record["detail"] = {
                "reason": (
                    f"the {self.options.flavor} surface publishes no accessibility tree "
                    f"and no anchor is calibrated for {action.argument!r}"
                )
            }
            return record
        width, height = self.screen
        bounds = self._content_bounds()
        tried: list[dict[str, object]] = []
        for index, (x, y) in enumerate(anchor.points(width, height)):
            self.adb.tap(x, y)
            time.sleep(self.options.settle_seconds)
            probe = self._capture(directory, f"nav-{index}")
            verdict = compare_frames(before.path, probe.path, bounds=bounds)
            tried.append({"point": [x, y], **verdict})
            if verdict["corroborated"]:
                record["resolved_by"] = "anchor"
                record["detail"] = {
                    "anchor": anchor.source,
                    "point": [x, y],
                    "candidates": tried,
                }
                return record
        record["resolved_by"] = "unresolved"
        record["detail"] = {
            "reason": "no navigation candidate changed the content region",
            "anchor": anchor.source,
            "candidates": tried,
        }
        return record

    # -- state readback ------------------------------------------------------

    def _state_readback(self, kind: StateReadbackKind) -> dict[str, object]:
        assert self.adb is not None
        if kind is StateReadbackKind.NONE:
            return {}
        if kind is StateReadbackKind.NOTIFICATION_PERMISSION:
            state = self.adb.package_grant_state(self.package, NOTIFICATION_PERMISSION)
            return {
                "kind": kind.value,
                "value": state or "unknown",
                "permission": NOTIFICATION_PERMISSION,
            }
        connections = self.adb.vpn_connections(self.package)
        established = any("VPN" in line for line in connections)
        return {
            "kind": kind.value,
            "value": "established" if established else "not-established",
            "package": self.package,
            "detail": connections[:8],
        }

    # -- one step ------------------------------------------------------------

    def _run_step(self, scenario: Scenario) -> tuple[StepReceipt | None, dict[str, object]]:
        assert self.adb is not None and self.logcat is not None
        directory = self.run_directory / "steps" / scenario.step_id
        directory.mkdir(parents=True, exist_ok=True)

        before = self._capture(directory, "before")
        replicate = self._capture(directory, "before-replicate")
        self.logcat.read()

        actions = [
            self._perform(action, directory, before) for action in scenario.actions
        ]
        time.sleep(self.options.settle_seconds)
        after = self._capture(directory, "after")
        window, window_source = self._logcat_window()

        bounds = self._content_bounds() if self._uses_navigation(scenario) else None
        verdict = compare_frames(before.path, after.path, replicate.path, bounds)

        session = self.logcat.session_lines()
        matched = self._match_marker(scenario, session)
        logcat_path = directory / "logcat.txt"
        logcat_path.write_text("\n".join(session) + "\n", encoding="utf-8")
        logcat_evidence = LogcatEvidence(
            path=logcat_path,
            sha256=sha256_file(logcat_path),
            pattern=scenario.logcat_pattern,
            matched_lines=matched,
            line_count=len(session),
        )

        assertion = self._ui_assertion(scenario)
        readback = self._state_readback(scenario.state_readback)
        activity = {
            "resumed": self.adb.resumed_activity(),
            "window_id": self.adb.window_id(),
            "focused_window": self.adb.focused_window(),
            "app_pid": self.adb.app_process_id(self.package),
        }

        satisfied = {
            EvidenceKind.PIXEL_DELTA: bool(verdict["corroborated"]),
            EvidenceKind.LOGCAT_MARKER: bool(matched),
            EvidenceKind.UI_ASSERTION: bool((assertion or {}).get("matched")),
            EvidenceKind.STATE_READBACK: bool(readback.get("value")),
        }
        missing = [kind.value for kind in scenario.evidence if not satisfied[kind]]

        outcome = StepOutcome.PROVEN
        reason = ""
        if not window:
            outcome = StepOutcome.UNPROVEN
            reason = "the step produced no logcat evidence"
        elif missing:
            if EvidenceKind.PIXEL_DELTA in scenario.evidence and not satisfied[
                EvidenceKind.PIXEL_DELTA
            ]:
                outcome = StepOutcome.NO_OBSERVABLE_CHANGE
                reason = (
                    "the screenshot does not differ from the previous state "
                    f"({verdict['changed_pixels']} changed pixels, "
                    f"{verdict['required_pixels']} required)"
                )
            else:
                outcome = StepOutcome.UNPROVEN
                reason = f"the evidence {', '.join(missing)} was not observed"

        record = {
            "step_id": scenario.step_id,
            "business_id": scenario.business_id,
            "flavor": scenario.flavor,
            "outcome": outcome.value,
            "reason": reason,
            "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "actions": actions,
            "frames": [
                frame.as_record(ROOT)
                for frame in (before, replicate, after)
            ],
            "pixel_delta": verdict,
            "logcat": {
                "path": str((directory / "logcat.txt").relative_to(ROOT)),
                "sha256": logcat_evidence.sha256,
                "pattern": scenario.logcat_pattern,
                "matched_lines": list(matched),
                "line_count": len(session),
                "step_lines": len(window),
                "window_source": window_source,
            },
            "ui_assertion": assertion,
            "state_readback": readback,
            "activity": activity,
        }

        if outcome is not StepOutcome.PROVEN:
            return None, record

        receipt = StepReceipt(
            step_id=scenario.step_id,
            business_id=scenario.business_id,
            flavor=scenario.flavor,
            timestamp_utc=record["timestamp_utc"],
            frame=after,
            frames=(before, replicate, after),
            logcat=logcat_evidence,
            pixel_delta=verdict,
            evidence=scenario.evidence,
            ui_assertion=assertion,
            state_readback=readback or None,
            actions=tuple(actions),
            activity=activity,
        )
        return receipt, record

    def _logcat_window(self) -> tuple[tuple[str, ...], str]:
        """The logcat lines of the step.

        The device buffer is the primary source.  When the device said nothing
        at all during the step, the product process' own lines are used: they
        prove that the product was alive and talking while the step ran.
        """
        assert self.logcat is not None
        window = self.logcat.read()
        if window:
            return window, "device"
        assert self.adb is not None
        return self.adb.app_logcat(self.package), "app-process"

    def _uses_navigation(self, scenario: Scenario) -> bool:
        return any(
            action.kind in (ActionKind.TAP_NAV, ActionKind.TAP_ANCHOR)
            for action in scenario.actions
        )

    def _match_marker(self, scenario: Scenario, window: tuple[str, ...]) -> tuple[str, ...]:
        if not scenario.logcat_pattern:
            return ()
        try:
            pattern = re.compile(scenario.logcat_pattern)
        except re.error as error:
            raise FlowError(
                f"the step {scenario.step_id!r} declares an invalid marker: {error}"
            ) from error
        matched = [line for line in window if pattern.search(line)]
        return tuple(matched[:20])

    def _ui_assertion(self, scenario: Scenario) -> dict[str, object] | None:
        if not scenario.assert_text:
            return None
        assert self.adb is not None
        region = Region(scenario.assert_region)
        try:
            node = select_node(
                self._hierarchy(),
                (scenario.assert_text,),
                region,
                self.screen,
            )
        except (AdbError, ValueError):
            node = None
        if node is None:
            return {
                "labels": [scenario.assert_text],
                "region": region.value,
                "matched": False,
            }
        return {
            "labels": [scenario.assert_text],
            "region": region.value,
            "matched": True,
            "node": node.as_record(),
        }

    # -- the whole run -------------------------------------------------------

    def run(self) -> FlavorReceipt:
        """Drive every declared step of the flavor and collect the receipts."""
        self.prepare()
        assert self.adb is not None
        self.run_directory.mkdir(parents=True, exist_ok=True)

        receipts: list[StepReceipt] = []
        unproven: list[dict[str, object]] = []
        proven: set[str] = set()
        for scenario in self.scenarios.for_flavor(self.options.flavor):
            if scenario.requires and scenario.requires not in proven:
                unproven.append(
                    {
                        "step_id": scenario.step_id,
                        "business_id": scenario.business_id,
                        "flavor": scenario.flavor,
                        "outcome": StepOutcome.BLOCKED.value,
                        "reason": (
                            f"the prerequisite step {scenario.requires!r} was not proven"
                        ),
                        "timestamp_utc": datetime.datetime.now(
                            datetime.timezone.utc
                        ).isoformat(),
                    }
                )
                continue
            receipt, record = self._run_step(scenario)
            if receipt is not None:
                receipts.append(receipt)
                proven.add(scenario.step_id)
            else:
                unproven.append(record)

        apk_digest = sha256_file(self.options.apk)
        return FlavorReceipt(
            flavor=self.options.flavor,
            apk_path=self.options.apk,
            apk_sha256=apk_digest,
            apk_bytes=self.options.apk.stat().st_size,
            package=self.package,
            activity=self.component,
            device=self.device.as_record() if self.device else {},
            guest={
                "airplane_mode": True,
                "vpn_consent": "ignore",
                "notification_permission": "revoked",
            },
            steps=tuple(receipts),
            unproven=tuple(unproven),
        )

    def cleanup(self) -> None:
        """Leave the emulator the way the flow found it."""
        if self.adb is None:
            return
        if not self.options.keep_installed:
            self.adb.force_stop(self.package)


def git_provenance(root: pathlib.Path) -> dict[str, object]:
    """The repository state the evidence was produced from."""
    head = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    status = subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout
    return {
        "git_head": head,
        "git_dirty_sha256": sha256_file_text(status),
        "tool": "scripts/android/android_interaction_capture.py",
    }


def sha256_file_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()
