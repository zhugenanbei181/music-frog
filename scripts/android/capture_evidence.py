"""The typed evidence and the receipt writer of the Android capture flow.

A receipt is only written for a step that was *proven*: the screenshot really
changed, the logcat window carries the step's marker, the accessibility
assertion held and the business id is known.  Anything else is reported as an
unproven step and never becomes a receipt.
"""

from __future__ import annotations

import dataclasses
import enum
import json
import pathlib

from capture_scenarios import EvidenceKind

SCHEMA_VERSION = 1
RECEIPT_KIND = "android-interaction-receipts"


class StepOutcome(enum.Enum):
    """How a step ended.  Only ``PROVEN`` may become a receipt."""

    PROVEN = "proven"
    NO_OBSERVABLE_CHANGE = "no_observable_change"
    UNPROVEN = "unproven"
    BLOCKED = "blocked"


class ReceiptError(RuntimeError):
    """A receipt does not carry the facts it has to carry."""


@dataclasses.dataclass(frozen=True)
class Frame:
    """One captured screenshot."""

    name: str
    path: pathlib.Path
    sha256: str
    width: int
    height: int

    def as_record(self, root: pathlib.Path) -> dict[str, object]:
        return {
            "name": self.name,
            "path": str(self.path.relative_to(root)),
            "sha256": self.sha256,
            "width": self.width,
            "height": self.height,
        }


@dataclasses.dataclass(frozen=True)
class LogcatEvidence:
    """The logcat window of one step and the lines that matched its marker."""

    path: pathlib.Path
    sha256: str
    pattern: str
    matched_lines: tuple[str, ...]
    line_count: int

    def as_record(self, root: pathlib.Path) -> dict[str, object]:
        return {
            "path": str(self.path.relative_to(root)),
            "sha256": self.sha256,
            "pattern": self.pattern,
            "matched_lines": list(self.matched_lines),
            "line_count": self.line_count,
        }


@dataclasses.dataclass(frozen=True)
class StepReceipt:
    """One proven interaction, bound to its APK, its business and its pixels."""

    step_id: str
    business_id: str
    flavor: str
    timestamp_utc: str
    frame: Frame
    frames: tuple[Frame, ...]
    logcat: LogcatEvidence
    pixel_delta: dict[str, object]
    evidence: tuple[EvidenceKind, ...]
    ui_assertion: dict[str, object] | None
    state_readback: dict[str, object] | None
    actions: tuple[dict[str, object], ...]
    activity: dict[str, object]

    def as_record(self, root: pathlib.Path) -> dict[str, object]:
        return {
            "feature_id": self.business_id,
            "business_id": self.business_id,
            "surface": self.flavor,
            "step_id": self.step_id,
            "timestamp_utc": self.timestamp_utc,
            "image": str(self.frame.path.relative_to(root)),
            "sha256": self.frame.sha256,
            "marker": str(self.logcat.path.relative_to(root)),
            "marker_sha256": self.logcat.sha256,
            "pixel_geometry": {"width": self.frame.width, "height": self.frame.height},
            "pixel_delta": self.pixel_delta,
            "frames": [frame.as_record(root) for frame in self.frames],
            "logcat": self.logcat.as_record(root),
            "evidence": [kind.value for kind in self.evidence],
            "ui_assertion": self.ui_assertion,
            "state_readback": self.state_readback,
            "actions": list(self.actions),
            "activity": self.activity,
        }


@dataclasses.dataclass(frozen=True)
class FlavorReceipt:
    """Every proven step of one flavor, bound to one APK."""

    flavor: str
    apk_path: pathlib.Path
    apk_sha256: str
    apk_bytes: int
    package: str
    activity: str
    device: dict[str, str]
    guest: dict[str, object]
    steps: tuple[StepReceipt, ...]
    unproven: tuple[dict[str, object], ...]

    def as_record(self, root: pathlib.Path) -> dict[str, object]:
        return {
            "flavor": self.flavor,
            "apk": {
                "path": str(self.apk_path.relative_to(root)),
                "sha256": self.apk_sha256,
                "bytes": self.apk_bytes,
            },
            "package": self.package,
            "activity": self.activity,
            "device": self.device,
            "guest": self.guest,
            "steps": [step.as_record(root) for step in self.steps],
            "unproven": list(self.unproven),
        }


def validate_flavor_receipt(receipt: FlavorReceipt, root: pathlib.Path) -> list[str]:
    """Everything a flavor receipt has to carry before it may be written."""
    problems: list[str] = []
    if not receipt.apk_sha256 or len(receipt.apk_sha256) != 64:
        problems.append(f"{receipt.flavor}: the APK digest is missing")
    if not receipt.apk_path.is_file():
        problems.append(f"{receipt.flavor}: the APK does not exist: {receipt.apk_path}")
    if not receipt.steps:
        problems.append(f"{receipt.flavor}: no step was proven")
    seen: set[str] = set()
    for step in receipt.steps:
        label = f"{receipt.flavor}/{step.step_id}"
        if step.step_id in seen:
            problems.append(f"{label}: the step is reported twice")
        seen.add(step.step_id)
        if not step.business_id:
            problems.append(f"{label}: the business id is missing")
        if not step.logcat.sha256 or not step.logcat.path.is_file():
            problems.append(f"{label}: the logcat evidence is missing")
        if step.logcat.line_count <= 0:
            problems.append(f"{label}: the logcat evidence is empty")
        if not step.frame.sha256 or not step.frame.path.is_file():
            problems.append(f"{label}: the screenshot is missing")
        if (
            EvidenceKind.PIXEL_DELTA in step.evidence
            and not step.pixel_delta.get("corroborated")
        ):
            problems.append(f"{label}: the screenshot proves no interaction")
        if EvidenceKind.LOGCAT_MARKER in step.evidence and not step.logcat.matched_lines:
            problems.append(f"{label}: the logcat marker was never observed")
        if EvidenceKind.UI_ASSERTION in step.evidence:
            assertion = step.ui_assertion or {}
            if not assertion.get("matched"):
                problems.append(f"{label}: the screen assertion did not hold")
        if EvidenceKind.STATE_READBACK in step.evidence:
            readback = step.state_readback or {}
            if not readback.get("value"):
                problems.append(f"{label}: the state readback produced no value")
        for path in (step.frame.path, step.logcat.path):
            try:
                path.relative_to(root)
            except ValueError:
                problems.append(f"{label}: the artefact escapes the repository: {path}")
    return problems


def build_document(
    root: pathlib.Path,
    receipts: list[FlavorReceipt],
    provenance: dict[str, object],
    scope: str = "flavor",
) -> dict[str, object]:
    """Build the machine readable summary, refusing an unprovable receipt set."""
    problems: list[str] = []
    for flavor_receipt in receipts:
        problems.extend(validate_flavor_receipt(flavor_receipt, root))
    if problems:
        raise ReceiptError(
            "the receipt set is not provable:\n  " + "\n  ".join(problems)
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "kind": RECEIPT_KIND,
        "scope": scope,
        "provenance": provenance,
        "flavors": [item.as_record(root) for item in receipts],
        "receipts": [
            _step_record(step, item, root)
            for item in receipts
            for step in item.steps
        ],
    }


def _step_record(
    step: StepReceipt,
    receipt: FlavorReceipt,
    root: pathlib.Path,
) -> dict[str, object]:
    """One step record, bound to the APK it was captured from."""
    record = step.as_record(root)
    record["apk_path"] = str(receipt.apk_path.relative_to(root))
    record["apk_sha256"] = receipt.apk_sha256
    record["package"] = receipt.package
    record["activity_component"] = receipt.activity
    return record


def write_document(
    document: dict[str, object],
    destination: pathlib.Path,
) -> dict[str, pathlib.Path]:
    """Write the summary document and its two tables."""
    destination.mkdir(parents=True, exist_ok=True)
    document_path = destination / "android-visual-receipts.json"
    document_path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")

    manifest_path = destination / "capture-manifest.tsv"
    with manifest_path.open("w", encoding="utf-8", newline="") as file:
        file.write("scenario\tstatus\tsha256\tapp_pid\twindow_id\tbusiness_id\tapk_sha256\n")
        for receipt in document["receipts"]:
            file.write(
                "\t".join(
                    (
                        f"{receipt['surface']}-{receipt['step_id']}",
                        "ok",
                        receipt["sha256"],
                        str(receipt["activity"].get("app_pid", "")),
                        str(receipt["activity"].get("window_id", "")),
                        receipt["business_id"],
                        receipt["apk_sha256"],
                    )
                )
                + "\n"
            )

    summary_path = destination / "receipts.tsv"
    with summary_path.open("w", encoding="utf-8", newline="") as file:
        file.write(
            "flavor\tstep_id\tbusiness_id\tstatus\tapk_sha256\tscreenshot_sha256"
            "\tchanged_pixels\tlogcat_matched\n"
        )
        for receipt in document["receipts"]:
            file.write(
                "\t".join(
                    (
                        receipt["surface"],
                        receipt["step_id"],
                        receipt["business_id"],
                        "proven",
                        receipt["apk_sha256"],
                        receipt["sha256"],
                        str(receipt["pixel_delta"].get("changed_pixels", "")),
                        str(len(receipt["logcat"]["matched_lines"])),
                    )
                )
                + "\n"
            )

    return {
        "document": document_path,
        "manifest": manifest_path,
        "summary": summary_path,
    }


def merge_documents(
    documents: list[dict[str, object]],
    destination: pathlib.Path,
) -> dict[str, pathlib.Path]:
    """Merge the per flavor documents into one receipt set."""
    merged: dict[str, object] = {
        "schema_version": SCHEMA_VERSION,
        "kind": RECEIPT_KIND,
        "scope": "merged",
        "provenance": {},
        "flavors": [],
        "receipts": [],
    }
    for document in documents:
        if document.get("schema_version") != SCHEMA_VERSION:
            raise ReceiptError("a receipt document has an unknown schema version")
        if document.get("kind") != RECEIPT_KIND:
            raise ReceiptError("a receipt document is not an Android receipt document")
        merged["provenance"] = document.get("provenance", {})
        merged["flavors"].extend(document.get("flavors", []))
        merged["receipts"].extend(document.get("receipts", []))
    if not merged["receipts"]:
        raise ReceiptError("the merged receipt set is empty")
    return write_document(merged, destination)
