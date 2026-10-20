#!/usr/bin/env python3
"""Guard the Android interaction capture flow and the receipts it writes.

Two things are guarded:

1. the *flow* — it drives a disposable emulator only, it never selects a
   physical device, it never touches the network, and its scenario table binds
   every step to a business of the shared parity catalogue;
2. the *receipts* — a receipt has to carry the digest of the APK it came
   from, the business it exercises, the screenshot, the logcat window that
   proves the interaction happened and the measured pixel change.  A receipt
   without one of these facts, or a receipt set whose screenshots prove
   nothing, is refused.

Usage
-----
    python3 scripts/quality/android-interaction-receipt-guard.py --mode enforce
    python3 scripts/quality/android-interaction-receipt-guard.py --self-test
    python3 scripts/quality/android-interaction-receipt-guard.py --receipts <document>
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import pathlib
import re
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]

FLOW = pathlib.Path("scripts/android/android_interaction_capture.py")
SCENARIOS = pathlib.Path("scripts/android/android_interaction_scenarios.tsv")
ANCHORS = pathlib.Path("scripts/android/android_interaction_anchors.tsv")
CATALOGUE = pathlib.Path("scripts/parity/feature_evidence.tsv")

RECEIPT_SCHEMA_VERSION = 1
RECEIPT_KIND = "android-interaction-receipts"

DIGEST = re.compile(r"^[0-9a-f]{64}$")

# The steps every flavor has to declare.
REQUIRED_STEPS = (
    "startup",
    "navigation",
    "list-scroll",
    "vpn-consent",
    "permission-prompt",
    "vpn-terminal-state",
)

# The network client the flow must never use.
FORBIDDEN_NETWORK = (
    "import requests",
    "import urllib",
    "from urllib",
    "import http",
    "from http",
    "import socket",
    "from socket",
    "import ftplib",
    "curl ",
    "wget ",
)


def _load_module(name: str, path: pathlib.Path):
    """Load one of the flow modules by path."""
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:  # pragma: no cover - defensive
        raise ImportError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def catalogue_business_ids(root: pathlib.Path = ROOT) -> set[str]:
    """The businesses the shared parity catalogue knows."""
    path = root / CATALOGUE
    if not path.is_file():
        return set()
    identifiers: set[str] = set()
    with path.open(encoding="utf-8", newline="") as file:
        for line in file.read().splitlines()[1:]:
            if not line.strip():
                continue
            identifiers.add(line.split("\t", 1)[0].strip())
    return identifiers


def scenario_problems(root: pathlib.Path = ROOT) -> list[str]:
    """The scenario table has to bind every step to a catalogue business."""
    problems: list[str] = []
    path = root / SCENARIOS
    if not path.is_file():
        return [f"{SCENARIOS}: the scenario table is missing"]
    sys.path.insert(0, str(root / "scripts/android"))
    try:
        scenarios = _load_module("capture_scenarios", path.parent / "capture_scenarios.py")
        table = scenarios.load_scenarios(path)
    except Exception as error:  # pragma: no cover - defensive
        return [f"{SCENARIOS}: the scenario table cannot be read: {error}"]

    catalogue = catalogue_business_ids(root)
    if not catalogue:
        problems.append(f"{CATALOGUE}: the parity catalogue is missing")

    for business in table.business_ids():
        if business not in catalogue:
            problems.append(
                f"{SCENARIOS}: the business {business!r} is not part of the parity catalogue"
            )
    for step in table.step_ids:
        if step not in REQUIRED_STEPS:
            problems.append(f"{SCENARIOS}: the step {step!r} is not a declared step")
    for step in REQUIRED_STEPS:
        if step not in table.step_ids:
            problems.append(f"{SCENARIOS}: the step {step!r} is missing")

    bindings: dict[str, set[str]] = {}
    for scenario in table.scenarios:
        bindings.setdefault(scenario.step_id, set()).add(scenario.business_id)
    for step, businesses in sorted(bindings.items()):
        if len(businesses) != 1:
            problems.append(
                f"{SCENARIOS}: the step {step!r} is bound to several businesses: "
                f"{sorted(businesses)}"
            )

    for scenario in table.scenarios:
        if scenario.requires and scenario.requires not in table.step_ids:
            problems.append(
                f"{SCENARIOS}: the step {scenario.step_id!r} requires the unknown "
                f"step {scenario.requires!r}"
            )
        if scenario.requires == scenario.step_id:
            problems.append(
                f"{SCENARIOS}: the step {scenario.step_id!r} requires itself"
            )
    return problems


def flow_problems(root: pathlib.Path = ROOT) -> list[str]:
    """The flow may only drive an emulator and may never touch the network."""
    problems: list[str] = []
    directory = root / "scripts/android"
    if not directory.is_dir():
        return ["scripts/android: the capture flow is missing"]
    for path in sorted(directory.glob("*.py")):
        source = path.read_text(encoding="utf-8")
        for forbidden in FORBIDDEN_NETWORK:
            if forbidden in source:
                problems.append(
                    f"{path.relative_to(root)}: the flow must not use {forbidden.strip()!r}"
                )

    try:
        device = _load_module("capture_device", directory / "capture_device.py")
    except Exception as error:  # pragma: no cover - defensive
        return [f"scripts/android/capture_device.py: cannot be loaded: {error}"]

    if not device.is_emulator_serial("emulator-5554"):
        problems.append("the flow must accept an emulator console serial")
    for serial in ("R5CT30ABCDE", "0123456789ABCDEF", "localhost:5555", ""):
        if device.is_emulator_serial(serial):
            problems.append(f"the flow must refuse the physical device serial {serial!r}")
    try:
        device.select_emulator_device(
            [device.DeviceIdentity(serial="R5CT30ABCDE", state="device")]
        )
    except device.DeviceSelectionError:
        pass
    else:
        problems.append("the flow must refuse to drive a physical device")
    return problems


def _artefact_problems(root: pathlib.Path, receipt: dict, label: str) -> list[str]:
    problems: list[str] = []
    image = receipt.get("image")
    if not isinstance(image, str) or not image:
        return [f"{label}: the receipt carries no screenshot"]
    path = root / image
    if not path.is_file():
        return [f"{label}: the screenshot does not exist: {image}"]
    if sha256_file(path) != receipt.get("sha256"):
        problems.append(f"{label}: the screenshot was altered")
    if path.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
        problems.append(f"{label}: the screenshot is not a PNG")
    return problems


def receipt_problems(root: pathlib.Path, document: dict) -> list[str]:
    """Every receipt has to carry the facts that bind it to the product."""
    problems: list[str] = []
    if document.get("schema_version") != RECEIPT_SCHEMA_VERSION:
        problems.append("the receipt document has an unknown schema version")
    if document.get("kind") != RECEIPT_KIND:
        problems.append("the receipt document is not an Android receipt document")
    receipts = document.get("receipts")
    if not isinstance(receipts, list) or not receipts:
        return problems + ["the receipt set is empty"]

    catalogue = catalogue_business_ids(root)
    scenarios = _scenario_flavor_pairs(root)
    seen: set[tuple[str, str]] = set()
    digests: set[str] = set()

    for receipt in receipts:
        label = f"{receipt.get('surface')}/{receipt.get('step_id')}"
        key = (str(receipt.get("surface")), str(receipt.get("step_id")))
        if key in seen:
            problems.append(f"{label}: the receipt is reported twice")
        seen.add(key)

        apk_digest = receipt.get("apk_sha256")
        if not isinstance(apk_digest, str) or not DIGEST.fullmatch(apk_digest or ""):
            problems.append(f"{label}: the receipt carries no APK digest")
        apk_path = receipt.get("apk_path")
        if not isinstance(apk_path, str) or not apk_path:
            problems.append(f"{label}: the receipt carries no APK path")
        else:
            candidate = root / apk_path
            if candidate.is_file() and sha256_file(candidate) != apk_digest:
                problems.append(f"{label}: the APK differs from the captured one")
        if isinstance(apk_digest, str):
            digests.add(apk_digest)

        business = receipt.get("business_id")
        if not business:
            problems.append(f"{label}: the receipt carries no business id")
        elif business not in catalogue:
            problems.append(f"{label}: the business {business!r} is not in the catalogue")
        if receipt.get("feature_id") != business:
            problems.append(f"{label}: the business id is not bound to the catalogue cell")

        logcat = receipt.get("logcat")
        if not isinstance(logcat, dict):
            problems.append(f"{label}: the receipt carries no logcat evidence")
        else:
            marker = logcat.get("path")
            if not isinstance(marker, str) or not marker:
                problems.append(f"{label}: the logcat evidence has no file")
            else:
                path = root / marker
                if not path.is_file():
                    problems.append(f"{label}: the logcat evidence does not exist: {marker}")
                else:
                    if sha256_file(path) != logcat.get("sha256"):
                        problems.append(f"{label}: the logcat evidence was altered")
                    if not path.read_text(encoding="utf-8", errors="replace").strip():
                        problems.append(f"{label}: the logcat evidence is empty")
            if "logcat-marker" in receipt.get("evidence", []) and not logcat.get(
                "matched_lines"
            ):
                problems.append(f"{label}: the logcat marker was never observed")

        delta = receipt.get("pixel_delta")
        if not isinstance(delta, dict):
            problems.append(f"{label}: the receipt carries no pixel measurement")
        elif "pixel-delta" in receipt.get("evidence", []):
            if not delta.get("corroborated"):
                problems.append(f"{label}: the screenshot proves no interaction")
            if int(delta.get("changed_pixels", 0)) < int(delta.get("required_pixels", 1)):
                problems.append(f"{label}: the pixel change does not beat the noise floor")

        if "ui-assertion" in receipt.get("evidence", []):
            assertion = receipt.get("ui_assertion")
            if not isinstance(assertion, dict) or not assertion.get("matched"):
                problems.append(f"{label}: the screen assertion did not hold")
        if "state-readback" in receipt.get("evidence", []):
            readback = receipt.get("state_readback")
            if not isinstance(readback, dict) or not readback.get("value"):
                problems.append(f"{label}: the state readback produced no value")

        for action in receipt.get("actions", []):
            if action.get("resolved_by") == "unresolved":
                problems.append(f"{label}: an unresolved action was reported as proven")
        problems.extend(_artefact_problems(root, receipt, label))

    if document.get("scope") == "merged":
        for flavor in sorted(scenarios):
            if not any(surface == flavor for surface, _ in seen):
                problems.append(
                    f"the merged receipt set carries no receipt for the {flavor} flavor"
                )

    if len(digests) == 1 and len(receipts) > 1:
        # One candidate APK for every flavor is allowed; the receipts still
        # have to differ from each other.
        pass
    screenshots = [entry.get("sha256") for entry in receipts]
    if len(set(screenshots)) == 1 and len(receipts) > 1:
        problems.append("the receipt set proves nothing: every screenshot is identical")
    return problems


def _scenario_flavor_pairs(root: pathlib.Path) -> set[str]:
    path = root / SCENARIOS
    if not path.is_file():
        return set()
    flavors: set[str] = set()
    with path.open(encoding="utf-8", newline="") as file:
        for line in file.read().splitlines()[1:]:
            if not line.strip():
                continue
            flavors.add(line.split("\t")[2].strip())
    return flavors


def _synthetic_receipt(root: pathlib.Path, directory: pathlib.Path) -> dict:
    """A receipt set that is provable in every respect."""
    png = bytes.fromhex(
        "89504e470d0a1a0a0000000d494844520000000100000001080600000"
        "01f15c4890000000a49444154789c6360000002000100ffff03000006000557bfabd40000000049454e44ae426082"
    )
    image = directory / "image.png"
    image.write_bytes(png)
    logcat = directory / "logcat.txt"
    logcat.write_text("I/MihomoApp: Initializing application...\n", encoding="utf-8")
    return {
        "schema_version": RECEIPT_SCHEMA_VERSION,
        "kind": RECEIPT_KIND,
        "scope": "flavor",
        "provenance": {},
        "flavors": [],
        "receipts": [
            {
                "feature_id": "runtime-telemetry-observation",
                "business_id": "runtime-telemetry-observation",
                "surface": "compose",
                "step_id": "startup",
                "image": str(image.relative_to(root)),
                "sha256": sha256_file(image),
                "logcat": {
                    "path": str(logcat.relative_to(root)),
                    "sha256": sha256_file(logcat),
                    "matched_lines": ["I/MihomoApp: Initializing application..."],
                    "line_count": 1,
                },
                "pixel_delta": {
                    "changed_pixels": 4096,
                    "compared_pixels": 1000,
                    "noise_pixels": 3,
                    "required_pixels": 16,
                    "corroborated": True,
                },
                "evidence": ["pixel-delta", "logcat-marker"],
                "ui_assertion": None,
                "state_readback": None,
                "actions": [{"kind": "launch", "resolved_by": "activity"}],
                "apk_path": str(image.relative_to(root)),
                "apk_sha256": sha256_file(image),
            }
        ],
    }


def self_test() -> int:
    """The guard accepts a provable receipt set and refuses every forgery."""
    ok = True
    with tempfile.TemporaryDirectory(prefix="android-receipt-guard-") as temporary:
        root = pathlib.Path(temporary)
        (root / "scripts/android").mkdir(parents=True)
        (root / "scripts/parity").mkdir(parents=True)
        for name in (
            "capture_actions.py",
            "capture_anchors.py",
            "capture_device.py",
            "capture_evidence.py",
            "capture_flow.py",
            "capture_hierarchy.py",
            "capture_pixels.py",
            "capture_scenarios.py",
        ):
            (root / "scripts/android" / name).write_text("", encoding="utf-8")
        (root / "scripts/parity/feature_evidence.tsv").write_text(
            "feature_id\tsurface\tlevel\tstatus\n"
            "runtime-telemetry-observation\tcompose\tcontract\tanchored\n",
            encoding="utf-8",
        )
        evidence = root / "evidence"
        evidence.mkdir()
        document = _synthetic_receipt(root, evidence)
        problems = receipt_problems(root, document)
        if problems:
            ok = False
            print(f"  [FAIL] a provable receipt set was refused: {problems}")
        else:
            print("  [ok] a provable receipt set is accepted")

        def expect(label: str, mutated: dict, needle: str) -> None:
            nonlocal ok
            found = receipt_problems(root, mutated)
            if not any(needle in issue for issue in found):
                ok = False
                print(f"  [FAIL] {label}: no problem mentions {needle!r}")
            else:
                print(f"  [ok] {label}")

        without_apk = json.loads(json.dumps(document))
        without_apk["receipts"][0]["apk_sha256"] = ""
        expect("a receipt without the APK digest", without_apk, "APK digest")

        without_business = json.loads(json.dumps(document))
        without_business["receipts"][0]["business_id"] = ""
        expect("a receipt without a business id", without_business, "business id")

        without_logcat = json.loads(json.dumps(document))
        without_logcat["receipts"][0]["logcat"] = {}
        expect("a receipt without the logcat evidence", without_logcat, "logcat evidence")

        static = json.loads(json.dumps(document))
        static["receipts"].append(json.loads(json.dumps(static["receipts"][0])))
        static["receipts"][1]["surface"] = "bevy"
        static["receipts"][1]["step_id"] = "navigation"
        expect("a receipt set that proves nothing", static, "identical")

        unproven = json.loads(json.dumps(document))
        unproven["receipts"][0]["pixel_delta"]["corroborated"] = False
        unproven["receipts"][0]["pixel_delta"]["changed_pixels"] = 0
        expect("a receipt without an interaction", unproven, "proves no interaction")

        unresolved = json.loads(json.dumps(document))
        unresolved["receipts"][0]["actions"] = [{"kind": "tap_text", "resolved_by": "unresolved"}]
        expect("a receipt with an unresolved action", unresolved, "unresolved action")

        empty = json.loads(json.dumps(document))
        empty["receipts"] = []
        expect("an empty receipt set", empty, "empty")

    if not ok:
        print("android interaction receipt guard self-test: FAILED", file=sys.stderr)
        return 1
    print("android interaction receipt guard self-test: passed")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--receipts", type=pathlib.Path)
    parser.add_argument("--root", default=str(ROOT))
    arguments = parser.parse_args(argv)

    if arguments.self_test:
        return self_test()

    root = pathlib.Path(arguments.root).resolve()
    problems = scenario_problems(root) + flow_problems(root)
    if arguments.receipts:
        document = json.loads(arguments.receipts.read_text(encoding="utf-8"))
        problems.extend(receipt_problems(root, document))
    if problems:
        for problem in problems:
            print(f"android interaction receipt guard: {problem}", file=sys.stderr)
        print(
            f"android interaction receipt guard: violations={len(problems)}",
            file=sys.stderr,
        )
        return 1 if arguments.mode == "enforce" else 0
    print("android interaction receipt guard: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
