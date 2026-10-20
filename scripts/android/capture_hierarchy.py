"""Read the Android UI hierarchy and resolve a node the flow can tap.

The Compose product publishes its semantics, and so do the system dialogs the
flow has to answer (the VPN consent dialog and the notification permission
dialog).  The native Bevy and Slint surfaces publish nothing, so their
navigation is resolved from the calibrated anchors instead.
"""

from __future__ import annotations

import dataclasses
import enum
import re
import xml.etree.ElementTree as ElementTree

BOUNDS = re.compile(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]")


class Region(enum.Enum):
    """Where a node has to live for the assertion to mean anything."""

    ANY = "any"
    TOP = "top"
    BOTTOM = "bottom"


@dataclasses.dataclass(frozen=True)
class HierarchyNode:
    """One node of the dumped accessibility tree."""

    text: str
    content_description: str
    resource_id: str
    class_name: str
    clickable: bool
    left: int
    top: int
    right: int
    bottom: int

    @property
    def width(self) -> int:
        return self.right - self.left

    @property
    def height(self) -> int:
        return self.bottom - self.top

    @property
    def center(self) -> tuple[int, int]:
        return (self.left + self.right) // 2, (self.top + self.bottom) // 2

    def labels(self) -> tuple[str, ...]:
        return tuple(
            label
            for label in (self.text, self.content_description, self.resource_id)
            if label
        )

    def as_record(self) -> dict[str, object]:
        return {
            "text": self.text,
            "content_description": self.content_description,
            "resource_id": self.resource_id,
            "class_name": self.class_name,
            "clickable": self.clickable,
            "bounds": [self.left, self.top, self.right, self.bottom],
            "center": list(self.center),
        }


def parse_hierarchy(document: str) -> list[HierarchyNode]:
    """Read the XML a `uiautomator dump` produced."""
    try:
        root = ElementTree.fromstring(document)
    except ElementTree.ParseError as error:
        raise ValueError(f"the UI hierarchy is not readable XML: {error}") from error
    nodes: list[HierarchyNode] = []
    for element in root.iter("node"):
        bounds = BOUNDS.match(element.get("bounds", ""))
        if bounds is None:
            continue
        nodes.append(
            HierarchyNode(
                text=element.get("text", ""),
                content_description=element.get("content-desc", ""),
                resource_id=element.get("resource-id", ""),
                class_name=element.get("class", ""),
                clickable=element.get("clickable", "false") == "true",
                left=int(bounds.group(1)),
                top=int(bounds.group(2)),
                right=int(bounds.group(3)),
                bottom=int(bounds.group(4)),
            )
        )
    return nodes


def _in_region(node: HierarchyNode, region: Region, screen: tuple[int, int]) -> bool:
    if region is Region.ANY:
        return True
    _, center_y = node.center
    height = screen[1] or 1
    if region is Region.TOP:
        return center_y <= height * 0.30
    return center_y >= height * 0.70


def _match_rank(node: HierarchyNode, wanted: tuple[str, ...]) -> int | None:
    """0 for an exact label, 1 for a label inside the node's text, None when absent."""
    labels = node.labels()
    if any(label in labels for label in wanted):
        return 0
    if any(label in text for text in labels for label in wanted):
        return 1
    return None


def find_nodes(
    nodes: list[HierarchyNode],
    labels: tuple[str, ...],
    region: Region = Region.ANY,
    screen: tuple[int, int] = (0, 0),
) -> list[HierarchyNode]:
    """Every node carrying one of the wanted labels inside the wanted region.

    An exact label always wins over a label that is only part of the node's
    text: a button called "Allow" beats the question that asks for "Allow".
    """
    wanted = tuple(label for label in labels if label)
    if not wanted:
        return []
    matches: list[tuple[int, int, int, int, HierarchyNode]] = []
    for node in nodes:
        if not _in_region(node, region, screen):
            continue
        rank = _match_rank(node, wanted)
        if rank is None:
            continue
        matches.append((rank, node.width * node.height, node.top, node.left, node))
    return [match[4] for match in sorted(matches, key=lambda match: match[:4])]


def select_node(
    nodes: list[HierarchyNode],
    labels: tuple[str, ...],
    region: Region = Region.ANY,
    screen: tuple[int, int] = (0, 0),
) -> HierarchyNode | None:
    """The single node the flow will tap, or ``None`` when it is ambiguous."""
    matches = find_nodes(nodes, labels, region, screen)
    if not matches:
        return None
    return matches[0]
