#!/usr/bin/env python3
"""CORE-SESSION: one owner for the Core session start, end to end.

The overview card shows a running session's length. That length used to be
missing at the bottom layer: the owner never published a session start, so
every surface either printed a dash or - worse - a duration it invented.

The chain is now typed from the owner down to the surface:

1. Contract owner:
   `crates/infiltrator-contract/src/snapshot.rs` owns
   `CoreSessionObservation` (`Unknown` / `Inactive` / `Active`) and
   `CoreSnapshot.session`.
2. Domain owner:
   `crates/infiltrator-domain/src/core_state.rs` carries
   `started_at_epoch_ms` in every session-bearing state, so a session start
   is born with its session token and dies with it.
3. Application owner:
   `crates/infiltrator-application/src/core_application/runtime_projection.rs`
   is the only place that reads the wall clock for the session: it stamps the
   start and measures the length at read time. No other application module
   may publish a session.
4. Presentation consumer:
   `crates/infiltrator-presentation/src/dashboard_hero_projection.rs` reads
   the typed observation and formats it through the shared helper. It never
   reads a clock and never writes a duration of its own.
5. One duration formatter:
   `crates/infiltrator-shared/src/elapsed_time.rs` is the only module that
   may format a duration. No other crate may define its own.
6. No fabricated duration:
   no crate may carry a hardcoded `00:00:00` session length.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
CONTRACT = ROOT / "crates/infiltrator-contract/src/snapshot.rs"
DOMAIN = ROOT / "crates/infiltrator-domain/src/core_state.rs"
APPLICATION = ROOT / "crates/infiltrator-application/src"
PROJECTION = ROOT / "crates/infiltrator-application/src/core_application/runtime_projection.rs"
PRESENTATION = ROOT / "crates/infiltrator-presentation/src/dashboard_hero_projection.rs"
SHARED = ROOT / "crates/infiltrator-shared/src/elapsed_time.rs"

SESSION_STATES = ("Starting", "Running", "Reloading", "Stopping", "Failed")
FABRICATED_DURATION = re.compile(r'"00:00:00"')
DURATION_FORMATTER = re.compile(
    r"fn\s+(?:format_duration|format_elapsed|format_uptime)\w*\s*\("
)
WALL_CLOCK = re.compile(r"SystemTime::now|Instant::now")
ACTIVE_SESSION = re.compile(r"CoreSessionObservation::Active")


def contract_violations(contract: pathlib.Path) -> list[str]:
    found: list[str] = []
    text = contract.read_text(encoding="utf-8")
    if "pub enum CoreSessionObservation" not in text:
        found.append("the contract no longer owns the typed session observation")
    for variant in ("Unknown", "Inactive", "Active"):
        if not re.search(rf"\b{variant}\b", text):
            found.append(f"the session observation lost its `{variant}` state")
    if not re.search(r"pub session:\s*CoreSessionObservation", text):
        found.append("the Core snapshot no longer publishes the session observation")
    return found


def domain_violations(domain: pathlib.Path) -> list[str]:
    found: list[str] = []
    text = domain.read_text(encoding="utf-8")
    declaration = re.search(r"pub enum CoreState\s*\{(?P<body>.*?)\n\}", text, re.S)
    if declaration is None:
        return ["the domain state machine no longer declares `CoreState`"]
    body = declaration.group("body")
    for state in SESSION_STATES:
        pattern = re.compile(rf"\b{state}\s*\{{(?P<fields>[^{{}}]*)\}}", re.S)
        match = pattern.search(body)
        if match is None:
            found.append(f"the domain state machine lost `CoreState::{state}`")
        elif "started_at_epoch_ms" not in match.group("fields"):
            found.append(
                f"`CoreState::{state}` no longer carries the session start with its token"
            )
    idle = re.search(r"\bIdle\s*\{(?P<fields>[^{}]*)\}", body, re.S)
    if idle is not None and "started_at_epoch_ms" in idle.group("fields"):
        found.append("`CoreState::Idle` must not carry a session start")
    events = text.split("pub enum CoreEvent", 1)[-1]
    if "started_at_epoch_ms" not in events:
        found.append("the start request no longer carries the session start")
    return found


def application_violations(application: pathlib.Path, projection: pathlib.Path) -> list[str]:
    found: list[str] = []
    projection_text = projection.read_text(encoding="utf-8")
    if "fn current_epoch_ms" not in projection_text:
        found.append("the Core owner no longer reads the clock it stamps the session with")
    if "CoreSessionObservation::Active" not in projection_text:
        found.append("the Core owner no longer publishes the running session")
    for path in sorted(application.rglob("*.rs")):
        if path == projection:
            continue
        text = path.read_text(encoding="utf-8")
        if ACTIVE_SESSION.search(text):
            found.append(
                f"{path.relative_to(ROOT)}: publishes a session outside the Core owner"
            )
    return found


def presentation_violations(presentation: pathlib.Path) -> list[str]:
    found: list[str] = []
    text = presentation.read_text(encoding="utf-8")
    if "snapshot.core.session" not in text:
        found.append("the projection no longer reads the session the owner published")
    if "format_elapsed_ms" not in text:
        found.append("the projection no longer formats the length through the shared helper")
    if WALL_CLOCK.search(text):
        found.append("the projection reads a wall clock instead of the owner's measurement")
    if FABRICATED_DURATION.search(text):
        found.append("the projection carries a fabricated session length")
    return found


def shared_violations(shared: pathlib.Path) -> list[str]:
    found: list[str] = []
    if not shared.is_file():
        return ["the shared duration formatter is missing"]
    text = shared.read_text(encoding="utf-8")
    if "pub fn format_elapsed_ms" not in text:
        found.append("the shared duration formatter lost its entry point")
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if path == shared:
            continue
        if "/lumen-source/" in str(path):
            continue
        text = path.read_text(encoding="utf-8")
        if DURATION_FORMATTER.search(text):
            found.append(
                f"{path.relative_to(ROOT)}: formats a duration outside the shared owner"
            )
    return found


def fabricated_duration_violations(shared: pathlib.Path) -> list[str]:
    found: list[str] = []
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if path == shared:
            continue
        if "/lumen-source/" in str(path):
            continue
        text = path.read_text(encoding="utf-8")
        if FABRICATED_DURATION.search(text):
            found.append(f"{path.relative_to(ROOT)}: carries a fabricated session length")
    return found


def violations(
    contract: pathlib.Path = CONTRACT,
    domain: pathlib.Path = DOMAIN,
    application: pathlib.Path = APPLICATION,
    projection: pathlib.Path = PROJECTION,
    presentation: pathlib.Path = PRESENTATION,
    shared: pathlib.Path = SHARED,
) -> list[str]:
    found: list[str] = []
    found.extend(contract_violations(contract))
    found.extend(domain_violations(domain))
    found.extend(application_violations(application, projection))
    found.extend(presentation_violations(presentation))
    found.extend(shared_violations(shared))
    found.extend(fabricated_duration_violations(shared))
    return found


ORIGINAL_READ_TEXT = pathlib.Path.read_text


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(), [])

    def _plant(self, path: pathlib.Path, planted: str) -> list[str]:
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(target: pathlib.Path, *args, **kwargs):
                if target == path:
                    return planted
                return ORIGINAL_READ_TEXT(target, *args, **kwargs)

            read_text.side_effect = fake_read_text
            return violations()

    def test_a_dropped_session_state_is_caught(self) -> None:
        planted = DOMAIN.read_text(encoding="utf-8").replace(
            """    Stopping {
        generation: u64,
        session_token: SessionToken,
        started_at_epoch_ms: u64,
    },""",
            """    Stopping {
        generation: u64,
        session_token: SessionToken,
    },""",
            1,
        )
        found = self._plant(DOMAIN, planted)
        self.assertTrue(
            any("no longer carries the session start" in item for item in found)
        )

    def test_a_projection_clock_is_caught(self) -> None:
        planted = (
            PRESENTATION.read_text(encoding="utf-8")
            + "\nfn projection_clock() -> u64 {\n    let _ = Instant::now();\n    0\n}\n"
        )
        found = self._plant(PRESENTATION, planted)
        self.assertTrue(any("reads a wall clock" in item for item in found))

    def test_a_second_duration_formatter_is_caught(self) -> None:
        planted = ROOT / "crates/infiltrator-iced/src/extra_duration.rs"
        planted.parent.mkdir(parents=True, exist_ok=True)
        planted.write_text(
            "pub fn format_duration_secs(secs: u64) -> String {\n    secs.to_string()\n}\n",
            encoding="utf-8",
        )
        try:
            found = violations()
        finally:
            planted.unlink()
        self.assertTrue(
            any("formats a duration outside the shared owner" in item for item in found)
        )

    def test_a_fabricated_length_is_caught(self) -> None:
        planted = PRESENTATION.read_text(encoding="utf-8") + '\nconst FAKE: &str = "00:00:00";\n'
        found = self._plant(PRESENTATION, planted)
        self.assertTrue(any("fabricated session length" in item for item in found))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        return 0 if unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful() else 1
    found = violations()
    for item in found:
        print(f"CORE-SESSION [{args.mode}]: {item}")
    print(f"core session guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
