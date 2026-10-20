"""The typed action vocabulary of the Android interaction capture flow.

A scenario row declares what the flow has to do; this module turns the
declaration into a typed value so an unknown or malformed action is a hard
error instead of a silently skipped step.
"""

from __future__ import annotations

import dataclasses
import enum


class ActionKind(enum.Enum):
    """Every interaction the flow knows how to perform."""

    LAUNCH = "launch"
    TAP_TEXT = "tap_text"
    TAP_NAV = "tap_nav"
    TAP_ANCHOR = "tap_anchor"
    SWIPE_UP = "swipe_up"
    SWIPE_DOWN = "swipe_down"
    WAIT_TEXT = "wait_text"
    TAP_DIALOG = "tap_dialog"
    REVOKE_PERMISSION = "revoke_permission"


# The action kinds that need an argument, and the separator between the kind
# and its argument.
ARGUMENT_KINDS = {
    ActionKind.TAP_TEXT,
    ActionKind.TAP_NAV,
    ActionKind.TAP_ANCHOR,
    ActionKind.WAIT_TEXT,
    ActionKind.TAP_DIALOG,
    ActionKind.REVOKE_PERMISSION,
}

# The separator between the alternatives of one label ("OK|Allow").
LABEL_SEPARATOR = "|"

# The separator between the actions of one step.
ACTION_SEPARATOR = ";"


class ActionError(ValueError):
    """The declared action is unknown or malformed."""


@dataclasses.dataclass(frozen=True)
class Action:
    """One declared interaction."""

    kind: ActionKind
    argument: str
    raw: str

    def labels(self) -> tuple[str, ...]:
        """The accepted labels of a text action."""
        if self.kind not in (ActionKind.TAP_TEXT, ActionKind.WAIT_TEXT, ActionKind.TAP_DIALOG):
            return ()
        return tuple(
            label.strip()
            for label in self.argument.split(LABEL_SEPARATOR)
            if label.strip()
        )


def parse_action(specification: str) -> Action:
    """Read one declared action."""
    raw = specification.strip()
    if not raw:
        raise ActionError("an action declaration is empty")
    name, separator, argument = raw.partition(":")
    try:
        kind = ActionKind(name.strip().lower())
    except ValueError as error:
        raise ActionError(f"unknown action {name!r} in {raw!r}") from error
    if kind in ARGUMENT_KINDS and not separator:
        raise ActionError(f"the action {kind.value!r} needs an argument in {raw!r}")
    if kind not in ARGUMENT_KINDS and separator:
        raise ActionError(f"the action {kind.value!r} takes no argument in {raw!r}")
    return Action(kind=kind, argument=argument.strip(), raw=raw)


def parse_actions(specification: str) -> tuple[Action, ...]:
    """Read the declared action sequence of one step."""
    parts = [part for part in specification.split(ACTION_SEPARATOR) if part.strip()]
    return tuple(parse_action(part) for part in parts)
