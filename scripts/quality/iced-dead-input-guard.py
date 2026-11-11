#!/usr/bin/env python3
"""ICED-001: a control whose handler is unconditionally `Message::Noop` cannot act.

The Slint tree has had this rule since SLINT-005 (`slint-dead-input-guard.py`):
an input nobody feeds renders the default and looks like working UI in a
screenshot. The Iced surface has the same failure mode one level down - a
control that is mounted, styled, localised and clickable, wired to a handler
that returns `Message::Noop` for every input. Two of them shipped on the
traffic page (the 1h/24h/7d/30d range pills and the Domains/Devices/Proxies/
Processes selector); both render a "selected" segment and do nothing, which is
exactly why the whole structure gate passed them.

The rule is deliberately narrow, because `Message::Noop` is legitimate in two
places: as the *fallback* of a gated action (`... .unwrap_or(Message::Noop)`
behind a `can_*` predicate, where the control is unavailable rather than
dead), and as the branch of an `if` that a real condition can leave. Both
answer a condition. A closure that ignores its argument and returns `Noop`
answers nothing - it is a control that can never act, whatever it renders.

Known offenders live in `iced_dead_input_inventory.json`, which may only
shrink: an entry whose site is gone is reported, so the ledger cannot rot into
a whitelist. Each entry names the task that owns the fix.

Usage:
    python3 scripts/quality/iced-dead-input-guard.py --self-test
    python3 scripts/quality/iced-dead-input-guard.py [--mode report|enforce]
    python3 scripts/quality/iced-dead-input-guard.py --update-inventory  # shrink only
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tempfile

REPO = pathlib.Path(__file__).resolve().parents[2]
VIEW = REPO / "crates/infiltrator-iced/src"
INVENTORY = REPO / "scripts/quality/iced_dead_input_inventory.json"

# A closure that throws its argument away and returns `Message::Noop` for every
# value: no input to the control can ever reach an owner.
INERT_HANDLER = re.compile(r"\|\s*_\s*\|\s*\{?\s*Message::Noop\b")

# The widgets that carry a *choice*: the user picks a value and the value has to
# go somewhere. `Message::Noop` in one of these is a dead control. The list is
# deliberately only this family - a fire-and-forget callback (`Task::perform`'s
# completion, which discards its own result) and an input-swallowing scrim
# (`.on_press(Message::Noop)`, whose whole purpose is to absorb clicks) are not
# choices a user can make, and neither is a defect.
CHOICE_WIDGETS = (
    "segmented_control_with_actions",
    "segmented_control",
    "pick_list",
    "combo_box",
    "checkbox",
    "toggler",
    "radio",
    "slider",
    "text_input",
    "text_editor",
)
NOT_A_CHOICE = "Task::perform"
LOOKBACK = 240
def inert_sites(text: str) -> list[int]:
    """Offsets of the inert handlers that sit on a choice widget's own call."""
    offsets: list[int] = []
    for match in INERT_HANDLER.finditer(text):
        # Only the statement the handler sits in may name the widget: a name
        # from an earlier line is somebody else's call.
        window = text[max(0, match.start() - LOOKBACK):match.start()]
        cut = max(window.rfind(delimiter) for delimiter in ";{}")
        window = window[cut + 1:]
        owners = [
            (window.rfind(name), name) for name in CHOICE_WIDGETS if window.rfind(name) >= 0
        ]
        if not owners:
            continue
        position, _name = max(owners)
        if window.rfind(NOT_A_CHOICE, position) > position:
            continue
        offsets.append(match.start())
    return offsets


def sources(root: pathlib.Path) -> list[pathlib.Path]:
    return sorted(path for path in root.rglob("*.rs") if path.is_file())


def detect(root: pathlib.Path) -> dict[str, int]:
    """Inert handlers per file, as a count (line numbers rot, counts do not)."""
    found: dict[str, int] = {}
    for path in sources(root):
        text = path.read_text(encoding="utf-8", errors="replace")
        count = len(inert_sites(text))
        if count:
            found[str(path.relative_to(REPO)) if path.is_relative_to(REPO) else path.name] = count
    return found


def read_inventory() -> list[dict]:
    if not INVENTORY.is_file():
        return []
    document = json.loads(INVENTORY.read_text(encoding="utf-8"))
    return list(document.get("sites", []))


def write_inventory(sites: list[dict]) -> None:
    INVENTORY.write_text(
        json.dumps({"version": 1, "sites": sites}, indent=2, ensure_ascii=False, sort_keys=True)
        + "\n",
        encoding="utf-8",
    )


def audit(root: pathlib.Path = VIEW, registered: list[dict] | None = None):
    """Return (unregistered sites, stale entries)."""
    registered = read_inventory() if registered is None else registered
    found = detect(root)
    budget: dict[str, int] = {}
    for entry in registered:
        budget[entry["file"]] = budget.get(entry["file"], 0) + 1
    findings = [
        (file, count - budget.get(file, 0))
        for file, count in sorted(found.items())
        if count > budget.get(file, 0)
    ]
    stale = [entry["file"] for entry in registered if found.get(entry["file"], 0) < budget[entry["file"]]]
    return findings, stale


def self_test() -> int:
    findings, stale = audit(VIEW, read_inventory())
    if findings or stale:
        print(f"SELF-TEST FAIL: live audit is not clean: unregistered={findings} stale={stale}")
        return 1
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        (root / "planted.rs").write_text(
            "\n".join(
                [
                    "fn a(labels: &[String]) {",
                    "    let dead = segmented_control(labels, 0, |_| Message::Noop);",
                    "    let gated = pick_list(options, selected, |o| {",
                    "        if can_apply { Message::Apply(o) } else { Message::Noop }",
                    "    });",
                    "    let fallback = build().unwrap_or(Message::Noop);",
                    "    let scrim = mouse_area(backdrop).on_press(|_| Message::Noop);",
                    "}",
                ]
            ),
            encoding="utf-8",
        )
        (root / "effect.rs").write_text(
            "\n".join(
                [
                    "fn b(ingress: Ingress) -> Task<Message> {",
                    "    Task::perform(async move { ingress.execute(x).await; }, |_| Message::Noop)",
                    "}",
                ]
            ),
            encoding="utf-8",
        )
        planted = detect(root)
        if planted != {"planted.rs": 1}:
            print(f"SELF-TEST FAIL: planted cases read {planted}, expected exactly one inert handler")
            return 1
        (root / "empty.rs").write_text("fn b() { let x = Message::Noop; }\n", encoding="utf-8")
        if "empty.rs" in detect(root):
            print("SELF-TEST FAIL: a bare Noop value was mistaken for an inert handler")
            return 1
    print(f"iced dead-input guard self-test: PASS (planted inert handler caught, {len(read_inventory())} live entries)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--update-inventory", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    registered = read_inventory()
    findings, stale = audit(VIEW, registered)

    if args.update_inventory:
        found = detect(VIEW)
        kept = [entry for entry in registered if found.get(entry["file"], 0) > 0]
        # The inventory may only shrink: a new inert control is a defect to fix,
        # not an entry to add.
        unregistered = {file: count for file, count in found.items()
                        if count > sum(1 for entry in kept if entry["file"] == file)}
        if unregistered:
            print(f"iced dead-input guard: refusing to register new inert controls: {unregistered}")
            return 1
        write_inventory(kept)
        print(f"iced dead-input guard: inventory now {len(kept)} site(s), dropped {len(registered) - len(kept)}")
        return 0

    for file, count in findings:
        print(f"ICED-001 [{file}] {count} control(s) handle every input with Message::Noop")
        for entry in registered:
            if entry["file"] == file:
                print(f"ICED-001 [{file}] registered owner: {entry.get('owner', '(none)')} - {entry.get('reason', '')}")
    for file in stale:
        print(f"ICED-001: inventory entry for {file} is stale - the inert control is gone, drop the entry")
    total = len(findings) + len(stale)
    print(f"iced dead-input guard: findings={total} registered={len(registered)}")
    return 1 if total and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
