#!/usr/bin/env python3
"""Guard FlClash's pinned DashboardWidget registry audit.

The desktop reference uses FlClash v0.9.3 as an external benchmark. Upstream
declares eleven dashboard card identities in `lib/enum/enum.dart`, maps each
one to a component in `lib/views/dashboard/widget_registry.dart`, and gives
two of them a platform gate. The registry also pins the single non-removable
card (`currentOutbound`) because it owns the start button.

`docs/reference/flclash-dashboard-widgets.tsv` is the in-repo, machine-readable
record of that comparison: every row names the one product truth source that
answers the same capability. This guard never clones upstream or compiles the
workspace; it keeps the pinned word list, platform decisions, registry
components and owner strings from drifting silently.
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
LEDGER = ROOT / "docs/reference/flclash-dashboard-widgets.tsv"
GATE = ROOT / "scripts/quality/check-structure.sh"

FIELDS = ("widget", "platforms", "removable", "component", "owner")
SHA_LINE = re.compile(r"^#\s+([0-9a-f]{64})\s*$", re.MULTILINE)
WIDGET_RE = re.compile(r"^[a-z][A-Za-z0-9]*$")

EXPECTED = {
    "currentOutbound": ("all", "false", "CurrentOutboundCard"),
    "networkSpeed": ("all", "true", "NetworkSpeedCard"),
    "outboundMode": ("all", "true", "OutboundModeCard"),
    "vpnButton": ("android", "true", "VpnCard"),
    "networkTakeover": ("desktop", "true", "NetworkTakeoverCard"),
    "serviceStatus": ("all", "true", "ServiceStatusCard"),
    "profiles": ("all", "true", "ProfilesCard"),
    "proxyGroups": ("all", "true", "ProxyGroupsCard"),
    "connectionStats": ("all", "true", "ConnectionStatsCard"),
    "intranetIp": ("all", "true", "IntranetIpCard"),
    "memoryInfo": ("all", "true", "MemoryInfo"),
}


def rows_of(text: str) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for raw in text.splitlines():
        if not raw or raw.lstrip().startswith("#"):
            continue
        if raw.split("\t", 1)[0] == FIELDS[0]:
            continue
        parts = raw.split("\t")
        if len(parts) != len(FIELDS):
            rows.append({"widget": raw, "platforms": "", "removable": "",
                         "component": "", "owner": "", "_malformed": "true"})
            continue
        rows.append(dict(zip(FIELDS, (part.strip() for part in parts))))
    return rows


def header_digest(text: str) -> str | None:
    match = SHA_LINE.search(text)
    return match.group(1) if match else None


def findings(text: str) -> list[str]:
    found: list[str] = []
    rows = rows_of(text)
    widgets = [row["widget"] for row in rows]
    digest = header_digest(text)
    if digest is None:
        found.append("the ledger header does not pin the upstream widget-list sha256")
    computed = hashlib.sha256(
        ("\n".join(sorted(widgets)) + "\n").encode("utf-8")).hexdigest()
    if digest is not None and computed != digest:
        found.append(
            f"the ledger's widget list hashes to {computed}, not the pinned {digest}; "
            "re-vendor deliberately and re-adjudicate the new cards")
    if len(rows) != len(EXPECTED):
        found.append(
            f"the pinned upstream has {len(EXPECTED)} DashboardWidgets, "
            f"the ledger has {len(rows)}")
    duplicates = sorted({name for name in widgets if widgets.count(name) > 1})
    if duplicates:
        found.append(f"duplicate widgets in the ledger: {duplicates}")
    for row in rows:
        name = row["widget"]
        if row.get("_malformed"):
            found.append(f"a ledger row is not five tab-separated fields: {name[:60]!r}")
            continue
        if not WIDGET_RE.match(name):
            found.append(f"{name!r} is not a DashboardWidget identifier")
        expected = EXPECTED.get(name)
        if expected is None:
            found.append(f"{name}: not in the pinned upstream DashboardWidget list")
            if row["owner"] == "unknown owner" or row["owner"].startswith("unknown"):
                found.append(f"{name}: the in-repo truth source is still unadjudicated")
            if not row["platforms"] or not row["removable"] or not row["component"]:
                found.append(f"{name}: an unregistered widget has incomplete registry facts")
            continue
        platforms, removable, component = expected
        if row["platforms"] != platforms:
            found.append(
                f"{name}: platform gate is {row['platforms']!r}, upstream says {platforms!r}")
        if row["removable"] != removable:
            found.append(
                f"{name}: removable is {row['removable']!r}, upstream says {removable!r}")
        if row["component"] != component:
            found.append(
                f"{name}: component is {row['component']!r}, registry says {component!r}")
        if not row["owner"]:
            found.append(f"{name}: no in-repo truth source named for the capability")
        if row["owner"] == "unknown owner" or row["owner"].startswith("unknown"):
            found.append(f"{name}: the in-repo truth source is still unadjudicated")
    for name in EXPECTED:
        if name not in widgets:
            found.append(f"{name}: pinned upstream DashboardWidget is missing")
    return found


def check() -> list[str]:
    if not LEDGER.is_file():
        return [f"{LEDGER.relative_to(ROOT)} is missing"]
    return findings(LEDGER.read_text(encoding="utf-8"))


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = LEDGER.read_text(encoding="utf-8")

    def test_live_ledger_is_clean(self) -> None:
        self.assertEqual(findings(self.text), [])

    def test_the_header_digest_is_the_sorted_widget_list(self) -> None:
        widgets = [row["widget"] for row in rows_of(self.text)]
        digest = hashlib.sha256(
            ("\n".join(sorted(widgets)) + "\n").encode("utf-8")).hexdigest()
        self.assertEqual(header_digest(self.text), digest)

    def test_a_dropped_widget_is_caught(self) -> None:
        planted = "\n".join(line for line in self.text.splitlines()
                            if not line.startswith("memoryInfo\t"))
        found = findings(planted)
        self.assertTrue(any("has 11 DashboardWidgets" in item for item in found), found)
        self.assertTrue(any("hashes to" in item for item in found), found)

    def test_an_unexpected_widget_is_caught(self) -> None:
        planted = self.text + "newWidget\tall\ttrue\tNewCard\tunknown owner\n"
        found = findings(planted)
        self.assertTrue(any("not in the pinned" in item for item in found), found)
        self.assertTrue(any("unadjudicated" in item for item in found), found)

    def test_a_platform_gate_drift_is_caught(self) -> None:
        planted = self.text.replace(
            "vpnButton\tandroid\t", "vpnButton\tall\t", 1)
        self.assertTrue(any("platform gate" in item for item in findings(planted)))

    def test_the_non_removable_card_is_caught(self) -> None:
        planted = self.text.replace(
            "currentOutbound\tall\tfalse\t", "currentOutbound\tall\ttrue\t", 1)
        self.assertTrue(any("removable is" in item for item in findings(planted)))

    def test_a_component_registry_drift_is_caught(self) -> None:
        planted = self.text.replace(
            "\tProfilesCard\t", "\tProfileCard\t", 1)
        self.assertTrue(any("registry says" in item for item in findings(planted)))

    def test_a_missing_owner_is_caught(self) -> None:
        planted = self.text.replace(
            "\tMemoryInfo\tinfiltrator-presentation::MemoryInfoCardProjection",
            "\tMemoryInfo\t", 1)
        self.assertTrue(any("no in-repo truth source" in item for item in findings(planted)))

    def test_a_malformed_row_is_caught(self) -> None:
        planted = self.text.replace("\t", " ", 1)
        self.assertTrue(any("five tab-separated" in item for item in findings(planted)))

    def test_the_guard_is_part_of_the_structure_gate(self) -> None:
        gate = GATE.read_text(encoding="utf-8")
        self.assertIn("flclash-dashboard-widget-guard.py --self-test", gate)
        self.assertIn("flclash-dashboard-widget-guard.py --mode enforce", gate)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    problems = check()
    for problem in problems:
        print(f"flclash dashboard widget guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(
        f"flclash dashboard widget guard: {len(rows_of(LEDGER.read_text(encoding='utf-8')))} "
        f"upstream widgets adjudicated, findings={len(problems)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
