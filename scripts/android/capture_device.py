"""ADB transport for the Android interaction capture flow.

The flow drives a disposable emulator and nothing else:

* only an emulator console serial (``emulator-<port>``) is accepted, so a
  physical device can never be selected;
* the transport never opens a network socket of its own — every command is an
  ``adb`` invocation against the local emulator console;
* the guest is kept in airplane mode, so the captured product cannot reach the
  network while the evidence is collected.
"""

from __future__ import annotations

import dataclasses
import pathlib
import re
import subprocess

# The emulator console port is the only serial shape this flow accepts.
EMULATOR_SERIAL = re.compile(r"^emulator-\d+$")

# The logcat buffers the flow reads.  ``events`` carries the framework's
# activity lifecycle facts, ``system`` the connectivity facts, ``main`` the
# product's own lines.
LOGCAT_BUFFERS = ("main", "events", "system")


class DeviceSelectionError(RuntimeError):
    """No usable emulator, or an ambiguous choice between emulators."""


class AdbError(RuntimeError):
    """An adb invocation failed or returned something unusable."""


@dataclasses.dataclass(frozen=True)
class DeviceIdentity:
    """What the flow knows about the device it drives."""

    serial: str
    state: str
    model: str = ""
    sdk: str = ""
    abi: str = ""

    def as_record(self) -> dict[str, str]:
        return {
            "serial": self.serial,
            "state": self.state,
            "model": self.model,
            "sdk": self.sdk,
            "abi": self.abi,
        }


def is_emulator_serial(serial: str) -> bool:
    """True only for an emulator console port."""
    return EMULATOR_SERIAL.fullmatch(serial.strip()) is not None


def list_devices(adb: pathlib.Path) -> list[DeviceIdentity]:
    """Every device adb knows about, emulators and physical ones alike."""
    command = [str(adb), "devices", "-l"]
    result = subprocess.run(command, capture_output=True, timeout=60, check=False)
    identities: list[DeviceIdentity] = []
    for line in result.stdout.decode("utf-8", "replace").splitlines()[1:]:
        fields = line.split()
        if len(fields) < 2:
            continue
        serial, state = fields[0], fields[1]
        model = ""
        for field in fields[2:]:
            if field.startswith("model:"):
                model = field.split(":", 1)[1]
        identities.append(DeviceIdentity(serial=serial, state=state, model=model))
    return identities


def select_emulator_device(devices: list[DeviceIdentity]) -> DeviceIdentity:
    """Pick the one running emulator, refusing everything else.

    A physical device is never returned: its serial is not an emulator port.
    """
    candidates = [
        device
        for device in devices
        if is_emulator_serial(device.serial) and device.state == "device"
    ]
    if not candidates:
        raise DeviceSelectionError(
            "no running emulator is attached; this flow never drives a physical device"
        )
    if len(candidates) > 1:
        names = ", ".join(device.serial for device in candidates)
        raise DeviceSelectionError(
            f"more than one emulator is attached ({names}); select one explicitly"
        )
    return candidates[0]


class Adb:
    """A narrow, emulator-only adb client."""

    def __init__(self, adb: pathlib.Path, serial: str, timeout: float = 120.0) -> None:
        if not is_emulator_serial(serial):
            raise DeviceSelectionError(
                f"{serial!r} is not an emulator serial; a physical device is refused"
            )
        self.adb = pathlib.Path(adb)
        self.serial = serial
        self.timeout = timeout

    # -- process plumbing ----------------------------------------------------

    def _run(
        self,
        arguments: list[str],
        timeout: float | None = None,
        check: bool = True,
    ) -> subprocess.CompletedProcess[bytes]:
        command = [str(self.adb), "-s", self.serial, *arguments]
        result = subprocess.run(
            command,
            capture_output=True,
            timeout=timeout or self.timeout,
            check=False,
        )
        if check and result.returncode != 0:
            detail = result.stderr.decode("utf-8", "replace").strip()
            raise AdbError(f"{' '.join(command)} failed: {detail}")
        return result

    def shell(self, *arguments: str, timeout: float | None = None) -> str:
        result = self._run(["shell", *arguments], timeout=timeout)
        return result.stdout.decode("utf-8", "replace")

    def exec_out(self, *arguments: str, timeout: float | None = None) -> bytes:
        return self._run(["exec-out", *arguments], timeout=timeout).stdout

    # -- device discovery ----------------------------------------------------

    def list_devices(self) -> list[DeviceIdentity]:
        return list_devices(self.adb)

    def describe_device(self, device: DeviceIdentity) -> DeviceIdentity:
        return dataclasses.replace(
            device,
            model=self._property("ro.product.model") or device.model,
            sdk=self._property("ro.build.version.sdk"),
            abi=self._property("ro.product.cpu.abi"),
        )

    def _property(self, name: str) -> str:
        return self.shell("getprop", name).strip()

    # -- screen and hierarchy ------------------------------------------------

    def screen_size(self) -> tuple[int, int]:
        raw = self.shell("wm", "size")
        match = re.search(r"(\d+)x(\d+)", raw)
        if match is None:
            raise AdbError(f"cannot read the screen size: {raw!r}")
        return int(match.group(1)), int(match.group(2))

    def density(self) -> int:
        raw = self.shell("wm", "density")
        match = re.search(r"(\d+)", raw)
        if match is None:
            raise AdbError(f"cannot read the screen density: {raw!r}")
        return int(match.group(1))

    def screenshot_png(self) -> bytes:
        return self.exec_out("screencap", "-p")

    def ui_hierarchy(self) -> str:
        raw = self.exec_out("uiautomator", "dump", "/dev/tty", timeout=180.0)
        text = raw.decode("utf-8", "replace")
        start = text.find("<?xml")
        end = text.rfind("</hierarchy>")
        if start < 0 or end < 0:
            raise AdbError("the UI hierarchy dump did not contain an XML document")
        return text[start : end + len("</hierarchy>")]

    # -- logcat --------------------------------------------------------------

    def app_logcat(self, package: str, tail: int = 200) -> tuple[str, ...]:
        """The product process' own log lines."""
        pid = self.app_process_id(package)
        if pid <= 0:
            return ()
        raw = self.exec_out(
            "logcat", "-d", "--pid", str(pid), "-v", "threadtime", timeout=120.0
        )
        return tuple(raw.decode("utf-8", "replace").splitlines()[-tail:])

    def logcat(self) -> str:
        arguments = ["logcat", "-d", "-v", "threadtime"]
        for buffer in LOGCAT_BUFFERS:
            arguments.extend(["-b", buffer])
        return self.exec_out(*arguments, timeout=180.0).decode("utf-8", "replace")

    # -- activity and window identity ---------------------------------------

    def resumed_activity(self) -> str | None:
        raw = self.shell("dumpsys", "activity", "activities", timeout=180.0)
        match = re.search(r"mResumedActivity:.*?u0\s+(\S+)", raw)
        if match is not None:
            return match.group(1)
        match = re.search(r"mResumedActivity:.*?(\S+/\S+)", raw)
        return match.group(1) if match is not None else None

    def focused_window(self) -> str | None:
        raw = self.shell("dumpsys", "window", "windows", timeout=180.0)
        match = re.search(r"mCurrentFocus=Window\{[^}]*\s+(\S+/\S+)\}", raw)
        if match is not None:
            return match.group(1)
        match = re.search(r"mCurrentFocus=Window\{([^}]*)\}", raw)
        return match.group(1).strip() if match is not None else None

    def window_id(self) -> str | None:
        raw = self.shell("dumpsys", "window", "windows", timeout=180.0)
        match = re.search(r"mCurrentFocus=Window\{([0-9a-fA-F]+)", raw)
        return match.group(1) if match is not None else None

    def package_grant_state(self, package: str, permission: str) -> str | None:
        raw = self.shell("dumpsys", "package", package, timeout=180.0)
        block = re.search(
            rf"permission {re.escape(permission)}:(.*?)(?:\n\s*\n|\Z)",
            raw,
            re.DOTALL,
        )
        if block is None:
            return None
        granted = re.search(r"granted=(\w+)", block.group(1))
        return granted.group(1) if granted is not None else None

    def vpn_connections(self, package: str) -> list[str]:
        raw = self.shell("dumpsys", "connectivity", timeout=180.0)
        return [line.strip() for line in raw.splitlines() if package in line]

    # -- device state --------------------------------------------------------

    def airplane_mode(self) -> bool:
        raw = self.shell("settings", "get", "global", "airplane_mode_on").strip()
        return raw == "1"

    def set_airplane_mode(self, enabled: bool) -> None:
        self.shell("cmd", "connectivity", "airplane-mode", "enable" if enabled else "disable")

    def install(self, apk: pathlib.Path) -> str:
        return self.shell("pm", "install", "-r", "-t", str(apk), timeout=900.0)

    def force_stop(self, package: str) -> None:
        self.shell("am", "force-stop", package)

    def app_process_id(self, package: str) -> int:
        raw = self.shell("pidof", package).strip()
        for token in raw.split():
            if token.isdigit():
                return int(token)
        return 0

    # -- interaction primitives ---------------------------------------------

    def launch(self, component: str, extras: dict[str, str] | None = None) -> str:
        command = ["am", "start", "-W", "-n", component]
        if extras:
            for key, value in extras.items():
                command.extend(["--es", key, str(value)])
        return self.shell(*command, timeout=180.0)

    def tap(self, x: int, y: int) -> None:
        self.shell("input", "tap", str(x), str(y))

    def swipe(self, x1: int, y1: int, x2: int, y2: int, milliseconds: int = 400) -> None:
        self.shell("input", "swipe", str(x1), str(y1), str(x2), str(y2), str(milliseconds))

    def revoke_permission(self, package: str, permission: str) -> None:
        self.shell("pm", "revoke", package, permission)

    def grant_permission(self, package: str, permission: str) -> None:
        self.shell("pm", "grant", package, permission)
