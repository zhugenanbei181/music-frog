"""The calibrated navigation anchors of the native Android surfaces.

The Bevy and Slint products render a native surface: they publish no
accessibility tree, so their bottom navigation cannot be found by label.  The
anchors below are fractions of the screen, derived from the product's own
layout rules and recorded with the source that defines them.  A flavor that has
no anchor is never guessed: the step reports itself as unproven.
"""

from __future__ import annotations

import csv
import dataclasses
import pathlib

COLUMNS = ("flavor", "name", "x_fraction", "y_fractions", "source")


class AnchorError(ValueError):
    """The anchor table is malformed."""


@dataclasses.dataclass(frozen=True)
class NavAnchor:
    """Where one control of a native surface lives on the screen."""

    flavor: str
    name: str
    x_fraction: float
    y_fractions: tuple[float, ...]
    source: str

    def points(self, width: int, height: int) -> list[tuple[int, int]]:
        """The candidate tap points of this item, from the most likely one."""
        x = round(width * self.x_fraction)
        return [(x, round(height * fraction)) for fraction in self.y_fractions]


class AnchorTable:
    """Every calibrated anchor of the native surfaces."""

    def __init__(self, anchors: list[NavAnchor]) -> None:
        self.anchors = tuple(anchors)

    def for_name(self, flavor: str, name: str) -> NavAnchor | None:
        for anchor in self.anchors:
            if anchor.flavor == flavor and anchor.name == name:
                return anchor
        return None


def load_anchors(path: pathlib.Path) -> AnchorTable:
    """Read the anchor table."""
    anchors: list[NavAnchor] = []
    with pathlib.Path(path).open(encoding="utf-8", newline="") as file:
        reader = csv.DictReader(file, delimiter="\t")
        if reader.fieldnames is None or tuple(reader.fieldnames) != COLUMNS:
            raise AnchorError(
                f"the anchor table must declare the columns {COLUMNS}, "
                f"found {reader.fieldnames}"
            )
        for number, row in enumerate(reader, start=2):
            flavor = (row["flavor"] or "").strip()
            name = (row["name"] or "").strip()
            if not flavor or not name:
                raise AnchorError(f"the anchor row {number} has no flavor or name")
            y_fractions = tuple(
                float(token.strip())
                for token in (row["y_fractions"] or "").split(",")
                if token.strip()
            )
            if not y_fractions:
                raise AnchorError(f"the anchor row {number} declares no y fraction")
            anchors.append(
                NavAnchor(
                    flavor=flavor,
                    name=name,
                    x_fraction=float((row["x_fraction"] or "0").strip()),
                    y_fractions=y_fractions,
                    source=(row["source"] or "").strip(),
                )
            )
    return AnchorTable(anchors)
