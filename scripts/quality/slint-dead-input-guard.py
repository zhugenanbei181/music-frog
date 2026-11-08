#!/usr/bin/env python3
"""SLINT-005: an `in property` nobody feeds is a defect, not a knob.

A Slint input whose default is a constant (or absent) and which no instantiation
site ever binds renders the *default*, silently. This surface has shipped that
bug twice in one session: `PageNetwork.core-version` printed "当前版本：" with
nothing after it, and `PageNetwork.proxy-recovery-pending` kept a live command's
button permanently disabled. Both looked like working UI in a screenshot.

The rule: every declared input must be bound at some instantiation site, written
by the component itself, or listed in ALLOWED_DEFAULTS with a reason. The list may
only shrink - an entry whose input has since been fed is reported, so the ledger
cannot rot into a silent whitelist.

Usage:
    python3 scripts/quality/slint-dead-input-guard.py --self-test
    python3 scripts/quality/slint-dead-input-guard.py [--mode report|enforce]
"""

from __future__ import annotations

import argparse
import pathlib
import re
import shutil
import sys
import tempfile

REPO = pathlib.Path(__file__).resolve().parents[2]
UI = REPO / "crates/infiltrator-slint/ui"

# Inputs that are deliberately left at their default by every caller. Each entry
# states why the default IS the decision; none of them may render a dead control.
ALLOWED_DEFAULTS = {
    # component -> [(prop, why the default is the decision)]
    "HomePolicyGroupGrid": [("show-header", "the grid always shows its header; the flag exists for a caller that has none yet")],
    "HomeRouteModePills": [("enabled", "egress mode is always settable; the owner answers when it cannot be applied")],
    "SectionHeading": [
        ("gap-above", "the heading owns its air from the token, no caller overrides it"),
        ("gap-below", "the heading owns its air from the token, no caller overrides it"),
    ],
    "SummaryRow": [("icon", "the summary rows in the connection detail card carry no icon")],
    "SectionCard": [
        ("inset", "one card geometry owner; the token is the decision, not a caller knob"),
        ("radius", "one card geometry owner; compact-layout derives the corner"),
    ],
    "SurfaceCard": [("inset", "one card geometry owner; the token is the decision, not a caller knob")],
}

def allowed(name: str, prop: str) -> bool:
    return any(p == prop for p, _reason in ALLOWED_DEFAULTS.get(name, []))

COMPONENT = re.compile(r"^(?:export )?component (\w+)", re.M)
INPUT = re.compile(r"^\s*in\s+(?:out\s+)?property\s*<([^>]+)>\s+([\w-]+)\s*(?::\s*(.+?))?\s*;\s*$")
CONSTANT = re.compile(r'^("[^"]*"|true|false|-?\d+(\.\d+)?|[A-Z][\w]*\.[\w-]+|)$')
BINDING = re.compile(r"\s*([\w-]+)\s*:(?!:)")
ALIAS = re.compile(r"\s*([\w-]+)\s*<=>")
HANDLER = re.compile(r"\s*([\w-]+)\s*(?:\([^)]*\))?\s*=>")
WRITE = re.compile(r"\s*(?:root|self)\.([\w-]+)\s*=")


def components(ui: pathlib.Path):
    for path in sorted(ui.rglob("*.slint")):
        text = path.read_text(encoding="utf-8")
        marks = [(m.group(1), m.start()) for m in COMPONENT.finditer(text)]
        for i, (name, start) in enumerate(marks):
            end = marks[i + 1][1] if i + 1 < len(marks) else len(text)
            yield name, path, text[start:end]


def _block_of(text: str, brace: int) -> str:
    depth, i = 0, brace
    while i < len(text):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[brace:i + 1]
        i += 1
    return ""


def bound_inputs(ui: pathlib.Path):
    """Every input name bound, aliased or handled at an instantiation site."""
    # The component registry is global on purpose: a component is almost always
    # instantiated in a different file from the one that declares it, so a
    # per-file check silently forgets every cross-file wiring - which is exactly
    # the case this rule has to see.
    names = {name for name, _path, _body in components(ui)}
    bound: dict[str, set[str]] = {}
    for path in sorted(ui.rglob("*.slint")):
        text = path.read_text(encoding="utf-8")
        for m in re.finditer(r"(\w+)\s*\{", text):
            comp = m.group(1)
            if comp not in names:
                continue
            block = _block_of(text, text.index("{", m.start()))
            block = block[block.index("{") + 1:]
            seen = bound.setdefault(comp, set())
            for stmt in re.split(r"[;\n]", block):
                for rx in (BINDING, ALIAS, HANDLER):
                    hit = rx.match(stmt)
                    if hit:
                        seen.add(hit.group(1))
    return bound


def audit(ui: pathlib.Path):
    """Return (findings, satisfied_allowances)."""
    bound = bound_inputs(ui)
    findings, satisfied = [], []
    for name, path, body in components(ui):
        if name not in bound:
            continue  # never instantiated: a different problem, not this rule
        fed = bound[name]
        for line in body.splitlines():
            m = INPUT.match(line)
            if not m:
                continue
            prop, default = m.group(2), (m.group(3) or "").strip()
            if prop in fed or WRITE.search(line) or re.search(r"\b" + re.escape(prop) + r"\s*=", body):
                continue
            if default and not CONSTANT.match(default):
                continue  # self-derived default: the component decides it itself
            if allowed(name, prop):
                satisfied.append((name, prop))
                continue
            findings.append((name, prop, default or "(no default)", path))
    stale = [
        (name, prop)
        for name, entries in ALLOWED_DEFAULTS.items()
        for prop, _reason in entries
        if (name, prop) not in satisfied
    ]
    return findings, stale


def self_test() -> int:
    findings, stale = audit(UI)
    if stale:
        print("SELF-TEST FAIL: allowance no longer needed: " + ", ".join(f"{n}.{p}" for n, p in stale))
        return 1
    # Plant the exact bug class: strip a binding the surface relies on and prove
    # the guard notices. Without this the guard could pass by matching nothing.
    with tempfile.TemporaryDirectory() as directory:
        copy = pathlib.Path(directory) / "ui"
        shutil.copytree(UI, copy)
        target = copy / "pages" / "pages_settings.slint"
        text = target.read_text(encoding="utf-8")
        needle = "        core-version: root.core-version;\n"
        if needle not in text:
            print("SELF-TEST FAIL: the planted defect no longer exists in pages_settings.slint")
            return 1
        target.write_text(text.replace(needle, "", 1))
        planted, _ = audit(copy)
        if not any(name == "PageNetwork" and prop == "core-version" for name, prop, _d, _p in planted):
            print("SELF-TEST FAIL: an unfed input went undetected")
            return 1
    print(f"slint dead-input guard self-test: PASS (planted defect caught, {len(findings)} live findings)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    findings, stale = audit(UI)
    for name, prop, default, path in findings:
        print(f"SLINT-005 [{path.name}] {name}.{prop} defaults to {default} and no instantiation site binds it")
    for name, prop in stale:
        print(f"SLINT-005: allowance for {name}.{prop} is stale - the input is fed now, drop the entry")
    total = len(findings) + len(stale)
    print(f"slint dead-input guard: findings={total} allowed={sum(len(v) for v in ALLOWED_DEFAULTS.values())}")
    return 1 if total and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
