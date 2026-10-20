#!/usr/bin/env python3
"""Semantic audit for implicit state machines.

``typed-state-guard.py`` is a bounded syntax reader: it recognises a narrow
naming shape and therefore reports zero findings even where a multi-stage
process hides behind ordinary fields. This audit closes that blind spot with
two mechanical shapes:

1. a *phase carrier* (a type whose name ends in ``State``/``Status``/
   ``Lifecycle``/``Phase``/``Session``/``Mode``) that stores two or more
   ``bool`` fields — their combination is a stage table;
2. a phase carrier that stores an ``Option<T>`` whose field name reads as a
   stage marker (``pending``, ``failure``, ``error``, ``active``, ...) — the
   option carries the stage instead of optional data.

The repository rule is that a multi-stage process is one typed value with
typed transitions, so every hit needs a human verdict. The verdicts live in
``scripts/quality/implicit-state-verdicts.json``:

* ``legitimate`` — the fields are independent facts, not a stage table;
* ``fixed`` — the type was rewritten to a typed stage;
* ``open`` — a confirmed violation that still has to be rewritten.

The audit fails when a candidate has no verdict (nobody decided yet) or when a
verdict is ``open``. A candidate that disappeared from the code is closed by
removing its verdict, so a fixed type cannot linger in the ledger.

Usage:
    python3 scripts/quality/implicit-state-audit.py --self-test
    python3 scripts/quality/implicit-state-audit.py --mode report
    python3 scripts/quality/implicit-state-audit.py --mode enforce
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERDICTS = Path("scripts/quality/implicit-state-verdicts.json")

# Field names that read as a stage marker rather than as optional data.
STAGE_WORDS = (
    "pending", "failure", "error", "active", "running", "busy", "loading",
    "in_flight", "inflight", "stale", "applied", "confirmed", "expired",
    "cancelled", "canceled", "completed", "started", "stopped", "dirty",
    "synced", "synchronized", "armed", "requested", "submitted", "composing",
)

CARRIER_SUFFIXES = ("State", "Status", "Lifecycle", "Phase", "Session", "Mode")

STRUCT_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?struct\s+([A-Za-z0-9_]+)\s*\{", re.MULTILINE)
# Fields are read without a line anchor so a single-line struct is seen too;
# the type list is bounded so a method body cannot masquerade as a field.
FIELD_RE = re.compile(
    r"(?<![A-Za-z0-9_])(?:pub(?:\([^)]*\))?\s+)?([A-Za-z0-9_]+)\s*:\s*"
    r"(bool|Option\s*<[^<>]*>|[A-Za-z0-9_:<>, ]+?)\s*(?:,|=|}|$)",
)
ENUM_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?enum\s+([A-Za-z0-9_]+)", re.MULTILINE)


@dataclass
class Candidate:
    kind: str
    file: str
    type: str
    fields: list[str] = field(default_factory=list)

    @property
    def key(self) -> str:
        # The key is repository relative so the ledger travels with the checkout.
        try:
            file_name = Path(self.file).relative_to(ROOT).as_posix()
        except ValueError:
            file_name = self.file
        return f"{file_name}::{self.type}::{self.kind}"


def strip_code(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.DOTALL)
    text = re.sub(r"//[^\n]*", " ", text)
    text = re.sub(r'"(?:\\.|[^"\\])*"', '""', text)
    return text


def struct_body(text: str, start: int) -> str:
    depth = 0
    for index in range(start, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[start + 1:index]
    return text[start:]


def production_sources(base: Path) -> list[Path]:
    sources: list[Path] = []
    if not base.exists():
        return sources
    for path in sorted(base.rglob("*.rs")):
        relative = path.relative_to(base).as_posix()
        if relative.startswith("tests/") or relative.endswith("/tests.rs"):
            continue
        if relative.endswith("_tests.rs") or relative.endswith("_test.rs"):
            # Test modules that live next to production sources are not product
            # surface and are not part of the implicit-state red line.
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        marker = text.find("#[cfg(test)]")
        if marker >= 0 and re.search(r"^\s*mod\s+\w+", text[marker:], re.MULTILINE):
            # The inline test module is excluded from the production surface.
            text = text[:marker]
        sources.append(path)
    return sources


def audit_file(path: Path) -> list[Candidate]:
    text = strip_code(path.read_text(encoding="utf-8", errors="replace"))
    candidates: list[Candidate] = []
    for match in STRUCT_RE.finditer(text):
        name = match.group(1)
        if not name.endswith(CARRIER_SUFFIXES):
            continue
        body = struct_body(text, match.end() - 1)
        bools: list[str] = []
        options: list[str] = []
        for field_match in FIELD_RE.finditer(body):
            field_name, type_text = field_match.group(1), field_match.group(2).strip()
            if type_text == "bool":
                bools.append(field_name)
            elif type_text.replace(" ", "").startswith("Option<"):
                options.append(field_name)
        stage_options = [
            name_ for name_ in options
            if any(word in name_.lower() for word in STAGE_WORDS)
        ]
        if len(bools) >= 2:
            candidates.append(Candidate("multi-bool", path.as_posix(), name, bools))
        if stage_options:
            candidates.append(Candidate("option-as-stage", path.as_posix(), name, stage_options))
    return candidates


def scan() -> list[Candidate]:
    candidates: list[Candidate] = []
    for base in (ROOT / "crates", ROOT / "android"):
        for path in production_sources(base):
            candidates.extend(audit_file(path))
    return candidates


def relative(path: str) -> str:
    try:
        return Path(path).relative_to(ROOT).as_posix()
    except ValueError:
        return path


def load_verdicts() -> dict[str, dict]:
    path = ROOT / VERDICTS
    if not path.exists():
        return {}
    document = json.loads(path.read_text(encoding="utf-8"))
    return {entry["key"]: entry for entry in document.get("candidates", [])}


def save_verdicts(entries: dict[str, dict]) -> None:
    path = ROOT / VERDICTS
    payload = {
        "schema_version": 1,
        "note": (
            "Every implicit-state candidate needs a human verdict. 'legitimate' means the "
            "fields are independent facts; 'fixed' means the type now carries one typed stage; "
            "'open' means a confirmed violation that still has to be rewritten."
        ),
        "candidates": sorted(entries.values(), key=lambda entry: entry["key"]),
    }
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def self_test() -> int:
    sample = """
struct GoodState { phase: Phase, count: usize }
struct FlagState { is_loading: bool, is_failed: bool }
struct OptState { pending_failure: Option<Failure> }
struct Facts { is_masked: bool, is_readonly: bool }
enum Phase { Idle, Busy }
"""
    text = strip_code(sample)
    found: list[tuple[str, str]] = []
    for match in STRUCT_RE.finditer(text):
        name = match.group(1)
        if not name.endswith(CARRIER_SUFFIXES):
            continue
        body = struct_body(text, match.end() - 1)
        bools = [m.group(1) for m in FIELD_RE.finditer(body) if m.group(2).strip() == "bool"]
        options = [
            m.group(1) for m in FIELD_RE.finditer(body)
            if m.group(2).strip().replace(" ", "").startswith("Option<")
        ]
        stage_options = [n for n in options if any(w in n.lower() for w in STAGE_WORDS)]
        if len(bools) >= 2:
            found.append((name, "multi-bool"))
        if stage_options:
            found.append((name, "option-as-stage"))
    assert found == [("FlagState", "multi-bool"), ("OptState", "option-as-stage")], found
    assert ENUM_RE.search(sample)
    print("implicit state audit self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--record", action="store_true", help="write the current candidates into the verdict ledger without verdicts")
    args = parser.parse_args()
    if args.self_test:
        return self_test()

    candidates = scan()
    ledger = load_verdicts()
    if args.record:
        for candidate in candidates:
            key = candidate.key
            if key not in ledger:
                ledger[key] = {
                    "key": key,
                    "file": relative(candidate.file),
                    "type": candidate.type,
                    "kind": candidate.kind,
                    "fields": candidate.fields,
                    "verdict": "",
                }
        save_verdicts(ledger)

    undecided = []
    open_items = []
    for candidate in candidates:
        entry = ledger.get(candidate.key)
        if entry is None or not entry.get("verdict"):
            undecided.append(candidate)
        elif entry["verdict"] == "open":
            open_items.append(candidate)
    stale = [key for key in ledger if key not in {c.key for c in candidates}]

    print(f"implicit state audit: candidates={len(candidates)} "
          f"undecided={len(undecided)} open={len(open_items)} stale_verdicts={len(stale)}")
    for candidate in undecided:
        print(f"  [undecided] {relative(candidate.file)}::{candidate.type} "
              f"({candidate.kind}: {', '.join(candidate.fields)})")
    for candidate in open_items:
        print(f"  [open] {relative(candidate.file)}::{candidate.type} "
              f"({candidate.kind}: {', '.join(candidate.fields)})")
    for key in stale:
        print(f"  [stale verdict] {key} — the type no longer matches a candidate; drop the verdict")
    if args.mode == "enforce" and (undecided or open_items or stale):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
