#!/usr/bin/env python3
"""Drive one Android flavor on an emulator and write its interaction receipts.

The flow never selects a physical device and never touches the network: it
drives the disposable emulator through adb, keeps the guest in airplane mode
and proves every interaction with the screenshot, the logcat window, the
accessibility assertion and the owner state readback.

Usage
-----
    python3 scripts/android/android_interaction_capture.py --flavor compose
    python3 scripts/android/android_interaction_capture.py --flavor bevy --serial emulator-5554
    python3 scripts/android/android_interaction_capture.py --merge .evidence/android/captures/<stamp>
    python3 scripts/android/android_interaction_capture.py --probe

Exit status
-----------
    0  every declared step of the flavor was proven
    1  at least one step was unproven; the receipts that were proven are kept
    2  the flow could not run at all
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from capture_anchors import load_anchors  # noqa: E402
from capture_device import (  # noqa: E402
    Adb,
    AdbError,
    DeviceIdentity,
    DeviceSelectionError,
    list_devices,
    select_emulator_device,
)
from capture_evidence import build_document, merge_documents, write_document  # noqa: E402
from capture_flow import CaptureFlow, FlowError, FlowOptions, git_provenance  # noqa: E402
from capture_scenarios import load_scenarios  # noqa: E402

FLAVORS = ("compose", "bevy", "slint")

DEFAULT_SCENARIOS = pathlib.Path(__file__).resolve().parent / "android_interaction_scenarios.tsv"
DEFAULT_ANCHORS = pathlib.Path(__file__).resolve().parent / "android_interaction_anchors.tsv"


def default_apk(flavor: str) -> pathlib.Path:
    return (
        ROOT
        / "android/app/build/outputs/apk"
        / flavor
        / "debug"
        / f"app-{flavor}-universal-debug.apk"
    )


def default_output() -> pathlib.Path:
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return ROOT / ".evidence/android/captures" / stamp


def default_adb() -> pathlib.Path:
    candidates: list[pathlib.Path] = []
    for variable in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        value = os.environ.get(variable)
        if value:
            candidates.append(pathlib.Path(value) / "platform-tools/adb")
    local = ROOT / "android/local.properties"
    if local.is_file():
        for line in local.read_text(encoding="utf-8").splitlines():
            key, separator, value = line.partition("=")
            if key.strip() == "sdk.dir" and separator:
                candidates.append(pathlib.Path(value.strip()) / "platform-tools/adb")
    candidates.append(pathlib.Path("/usr/bin/adb"))
    for candidate in candidates:
        if candidate.is_file() and candidate.stat().st_mode & 0o111:
            return candidate
    raise FlowError("no adb binary was found; pass --adb")


def write_report(
    flavor: str,
    records: list[dict[str, object]],
    destination: pathlib.Path,
) -> pathlib.Path:
    """The human readable ledger of what the flow proved and what it did not."""
    destination.mkdir(parents=True, exist_ok=True)
    report = destination / "report.md"
    lines = [
        f"# Android interaction capture — {flavor}",
        "",
        f"Generated (UTC): {datetime.datetime.now(datetime.timezone.utc).isoformat()}",
        "",
        "| Step | Business | Outcome | Reason |",
        "| --- | --- | --- | --- |",
    ]
    for record in records:
        reason = str(record.get("reason") or "")
        lines.append(
            f"| {record['step_id']} | {record['business_id']} | "
            f"{record['outcome']} | {reason} |"
        )
    lines.append("")
    report.write_text("\n".join(lines), encoding="utf-8")
    (destination / "report.json").write_text(
        json.dumps({"flavor": flavor, "steps": records}, indent=2) + "\n",
        encoding="utf-8",
    )
    return report


def probe(options: argparse.Namespace) -> int:
    """Report what the flow sees on the emulator; the calibration helper."""
    adb = Adb(options.adb, options.serial or "emulator-5554")
    devices = list_devices(options.adb)
    device = select_emulator_device(devices)
    adb = Adb(options.adb, device.serial)
    print(f"serial={device.serial}")
    print(f"screen={adb.screen_size()}")
    print(f"density={adb.density()}")
    print(f"airplane_mode={adb.airplane_mode()}")
    print(f"resumed_activity={adb.resumed_activity()}")
    print(f"focused_window={adb.focused_window()}")
    return 0


def merge(directories: list[pathlib.Path]) -> int:
    """Merge the per flavor receipt documents into one receipt set."""
    documents: list[dict[str, object]] = []
    for directory in directories:
        for path in sorted(directory.glob("*/android-visual-receipts.json")):
            documents.append(json.loads(path.read_text(encoding="utf-8")))
    if not documents:
        print("no per flavor receipt document was found", file=sys.stderr)
        return 1
    written = merge_documents(documents, directories[0])
    print(f"merged {len(documents)} flavor documents: {written['document']}")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--flavor", choices=FLAVORS)
    parser.add_argument("--apk", type=pathlib.Path)
    parser.add_argument("--output", type=pathlib.Path)
    parser.add_argument("--adb", type=pathlib.Path)
    parser.add_argument("--serial")
    parser.add_argument("--scenarios", type=pathlib.Path, default=DEFAULT_SCENARIOS)
    parser.add_argument("--anchors", type=pathlib.Path, default=DEFAULT_ANCHORS)
    parser.add_argument("--settle", type=float, default=1.5)
    parser.add_argument("--dialog-timeout", type=float, default=25.0)
    parser.add_argument("--keep-installed", action="store_true")
    parser.add_argument("--merge", nargs="+", type=pathlib.Path)
    parser.add_argument("--probe", action="store_true")
    arguments = parser.parse_args(argv)

    try:
        arguments.adb = arguments.adb or default_adb()
        if arguments.merge:
            return merge(arguments.merge)
        if arguments.probe:
            return probe(arguments)
        if not arguments.flavor:
            parser.error("--flavor is required unless --merge or --probe is used")

        output = (arguments.output or default_output()).resolve()
        if not output.is_relative_to(ROOT):
            raise FlowError(f"the evidence directory has to live inside the repository: {output}")
        apk = (arguments.apk or default_apk(arguments.flavor)).resolve()
        scenarios = load_scenarios(arguments.scenarios)
        anchors = load_anchors(arguments.anchors)
        options = FlowOptions(
            flavor=arguments.flavor,
            apk=apk,
            output=output,
            adb=arguments.adb,
            serial=arguments.serial,
            settle_seconds=arguments.settle,
            dialog_timeout_seconds=arguments.dialog_timeout,
            keep_installed=arguments.keep_installed,
        )
        flow = CaptureFlow(options, scenarios, anchors)
        receipt = flow.run()
    except (FlowError, DeviceSelectionError, AdbError) as error:
        print(f"android interaction capture: {error}", file=sys.stderr)
        return 2
    except Exception as error:  # the flow must never fail open
        print(f"android interaction capture: {error}", file=sys.stderr)
        return 2

    records = [
        {
            "step_id": step.step_id,
            "business_id": step.business_id,
            "outcome": "proven",
            "reason": "",
        }
        for step in receipt.steps
    ] + [dict(record) for record in receipt.unproven]
    order = {step_id: index for index, step_id in enumerate(scenarios.step_ids)}
    records.sort(key=lambda record: order.get(str(record["step_id"]), len(order)))
    report = write_report(arguments.flavor, records, output / arguments.flavor)

    if receipt.steps:
        document = build_document(ROOT, [receipt], git_provenance(ROOT))
        written = write_document(document, output / arguments.flavor)
        print(f"receipts: {written['document']}")
    else:
        print(
            "no step was proven; the receipt set stays empty on purpose",
            file=sys.stderr,
        )
    flow.cleanup()
    print(f"report: {report}")

    unproven = len(receipt.unproven)
    if unproven:
        print(
            f"android interaction capture: {unproven} step(s) were not proven",
            file=sys.stderr,
        )
        return 1
    print(
        f"android interaction capture: {len(receipt.steps)} step(s) proven for "
        f"{arguments.flavor}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
