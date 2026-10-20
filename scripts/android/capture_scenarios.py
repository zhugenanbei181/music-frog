"""The scenario table of the Android interaction capture flow.

One row is one (step, flavor) pair.  The table is the single source of truth
for three things: which business a step belongs to, what the flow has to do,
and which evidence the step has to produce before it may become a receipt.
"""

from __future__ import annotations

import csv
import dataclasses
import enum
import pathlib

from capture_actions import Action, parse_actions
from capture_hierarchy import Region

FLAVORS = ("compose", "bevy", "slint")

COLUMNS = (
    "step_id",
    "business_id",
    "flavor",
    "actions",
    "assert_text",
    "assert_region",
    "logcat_pattern",
    "evidence",
    "state_readback",
    "requires",
)


class EvidenceKind(enum.Enum):
    """What a step has to prove before it may become a receipt."""

    PIXEL_DELTA = "pixel-delta"
    LOGCAT_MARKER = "logcat-marker"
    UI_ASSERTION = "ui-assertion"
    STATE_READBACK = "state-readback"


class StateReadbackKind(enum.Enum):
    """Which owner state the flow has to read back after a step."""

    NONE = "none"
    VPN_SESSION = "vpn-session"
    NOTIFICATION_PERMISSION = "notification-permission"


class ScenarioError(ValueError):
    """The scenario table is malformed."""


@dataclasses.dataclass(frozen=True)
class Scenario:
    """One (step, flavor) row of the scenario table."""

    step_id: str
    business_id: str
    flavor: str
    actions: tuple[Action, ...]
    assert_text: str
    assert_region: str
    logcat_pattern: str
    evidence: tuple[EvidenceKind, ...]
    state_readback: StateReadbackKind
    requires: str

    @property
    def key(self) -> tuple[str, str]:
        return self.flavor, self.step_id


class ScenarioTable:
    """The whole scenario table, ordered by the declared step sequence."""

    def __init__(self, scenarios: list[Scenario]) -> None:
        self.scenarios = tuple(scenarios)
        seen: set[tuple[str, str]] = set()
        for scenario in self.scenarios:
            if scenario.key in seen:
                raise ScenarioError(
                    f"the scenario table declares {scenario.key} twice"
                )
            seen.add(scenario.key)
        self.step_ids = tuple(
            dict.fromkeys(scenario.step_id for scenario in self.scenarios)
        )

    def for_flavor(self, flavor: str) -> tuple[Scenario, ...]:
        if flavor not in FLAVORS:
            raise ScenarioError(f"unknown Android flavor {flavor!r}")
        return tuple(
            scenario for scenario in self.scenarios if scenario.flavor == flavor
        )

    def business_ids(self) -> tuple[str, ...]:
        return tuple(sorted({scenario.business_id for scenario in self.scenarios}))


def _evidence(raw: str, row: int) -> tuple[EvidenceKind, ...]:
    kinds: list[EvidenceKind] = []
    for token in raw.split(","):
        name = token.strip()
        if not name:
            continue
        try:
            kinds.append(EvidenceKind(name))
        except ValueError as error:
            raise ScenarioError(
                f"the scenario row {row} names the unknown evidence kind {name!r}"
            ) from error
    if not kinds:
        raise ScenarioError(f"the scenario row {row} declares no evidence")
    return tuple(kinds)


def _region(raw: str, row: int) -> str:
    name = raw.strip() or Region.ANY.value
    try:
        return Region(name).value
    except ValueError as error:
        raise ScenarioError(
            f"the scenario row {row} names the unknown region {name!r}"
        ) from error


def _state_readback(raw: str, row: int) -> StateReadbackKind:
    name = raw.strip() or StateReadbackKind.NONE.value
    try:
        return StateReadbackKind(name)
    except ValueError as error:
        raise ScenarioError(
            f"the scenario row {row} names the unknown state readback {name!r}"
        ) from error


def load_scenarios(path: pathlib.Path) -> ScenarioTable:
    """Read the scenario table."""
    scenarios: list[Scenario] = []
    with pathlib.Path(path).open(encoding="utf-8", newline="") as file:
        reader = csv.DictReader(file, delimiter="\t")
        if reader.fieldnames is None or tuple(reader.fieldnames) != COLUMNS:
            raise ScenarioError(
                f"the scenario table must declare the columns {COLUMNS}, "
                f"found {reader.fieldnames}"
            )
        for number, row in enumerate(reader, start=2):
            flavor = (row["flavor"] or "").strip()
            if flavor not in FLAVORS:
                raise ScenarioError(
                    f"the scenario row {number} names the unknown flavor {flavor!r}"
                )
            step_id = (row["step_id"] or "").strip()
            business_id = (row["business_id"] or "").strip()
            if not step_id or not business_id:
                raise ScenarioError(
                    f"the scenario row {number} needs a step id and a business id"
                )
            evidence = _evidence(row["evidence"] or "", number)
            state_readback = _state_readback(row["state_readback"] or "", number)
            actions = parse_actions(row["actions"] or "")
            if not actions and state_readback is StateReadbackKind.NONE:
                raise ScenarioError(
                    f"the scenario row {number} declares no action and reads no state back"
                )
            scenarios.append(
                Scenario(
                    step_id=step_id,
                    business_id=business_id,
                    flavor=flavor,
                    actions=actions,
                    assert_text=(row["assert_text"] or "").strip(),
                    assert_region=_region(row["assert_region"] or "any", number),
                    logcat_pattern=(row["logcat_pattern"] or "").strip(),
                    evidence=evidence,
                    state_readback=state_readback,
                    requires=(row["requires"] or "").strip(),
                )
            )
    if not scenarios:
        raise ScenarioError("the scenario table is empty")
    return ScenarioTable(scenarios)
