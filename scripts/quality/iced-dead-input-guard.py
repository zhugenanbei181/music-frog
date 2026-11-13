#!/usr/bin/env python3
"""ICED-001: an enabled control that can produce `Message::Noop` is inert.

The guard judges syntax in `crates/infiltrator-iced/src`, not Rust types and not
pixels. An always-on press/select/input handler is a finding if its argument can
produce `Noop`: a fallback, a Noop-only closure, a direct Noop message, or a
helper that builds such a message. A `Noop` branch in a handler that also has a
typed branch answers a real condition and is not an inert control. A completion
callback (`Task::perform`), a modal scrim that absorbs input, an update arm, and
a genuinely disabled `*_maybe(None)` control are not enabled input handlers.

Known offenders live in `iced_dead_input_inventory.json`, which may only
shrink: both a new site and a registered site that disappeared are findings,
so the ledger cannot rot into a whitelist. Each entry names its exact line,
syntactic form, owner, and reason.

Usage:
    python3 scripts/quality/iced-dead-input-guard.py --self-test
    python3 scripts/quality/iced-dead-input-guard.py [--mode report|enforce]
    python3 scripts/quality/iced-dead-input-guard.py --update-inventory  # shrink only
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import pathlib
import re
import shutil
import sys
import tempfile
import unittest

REPO = pathlib.Path(__file__).resolve().parents[2]
VIEW = REPO / "crates/infiltrator-iced/src"
INVENTORY = REPO / "scripts/quality/iced_dead_input_inventory.json"
QUALITY_ROOT = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(QUALITY_ROOT))
from rust_syntax import mask_noncode  # noqa: E402

NOOP = re.compile(r"\b(?:Message::)?Noop\b")
FUNCTION = re.compile(r"\bfn\s+(?P<name>[A-Za-z_][A-Za-z_0-9]*)\s*(?:<[^>]*>)?\s*\(")
CALL = re.compile(r"\b(?P<name>[A-Za-z_][A-Za-z_0-9]*)\s*\(")
WIDGETS = {
    "button", "icon_button", "segmented_control_with_actions", "segmented_control",
    "pick_list", "combo_box", "checkbox", "toggler", "radio", "slider",
    "text_input", "text_editor", "toggle_switch_with_actions", "toggle_switch",
}
CHOICE_WIDGETS = {
    "segmented_control_with_actions", "segmented_control", "pick_list",
    "combo_box", "checkbox", "toggler", "radio", "slider", "text_input",
    "text_editor", "toggle_switch_with_actions", "toggle_switch",
}
HANDLERS = {
    "on_press", "on_release", "on_input", "on_submit", "on_select",
    "on_toggle", "on_change", "on_scroll", "on_right_press",
    "on_middle_press", "on_press_with", "on_input_with",
}
MAYBE_HANDLERS = re.compile(r"\.\s*on_\w+_maybe\s*\(")
FALLBACK = re.compile(
    r"\.\s*(?:unwrap_or|unwrap_or_else)\s*\(", re.S
)
NOOP_CLOSURE = re.compile(r"\|[^|]*\|\s*(?:\{)?\s*(?:Message::)?Noop\b", re.S)
MESSAGE_HELPERS = re.compile(r"(?:message|action|intent|on_press|on_change|on_input)", re.I)
LEGITIMATE_FUNCTION = re.compile(
    r"(?:^|_)update(?:_|$)|(?:^|_)task(?:_|$)|(?:^|_)(?:scrim|backdrop)(?:_|$)"
)
MAX_SCAN = 1400


def mask(text: str) -> str:
    """Blank comments and string literals without changing offsets or lines."""
    masked = mask_noncode(text)
    return "".join("\n" if char == "\n" else " " if masked[index] == " " else char
                   for index, char in enumerate(text))


def matching_paren(text: str, start: int) -> int:
    depth = 0
    for position in range(start, len(text)):
        if text[position] == "(":
            depth += 1
        elif text[position] == ")":
            depth -= 1
            if depth == 0:
                return position
    return len(text) - 1


def function_span(text: str, declaration: re.Match) -> tuple[int, int] | None:
    """The body of one `fn` declaration, found by brace balance."""
    open_brace = text.find("{", declaration.end())
    if open_brace < 0:
        return None
    depth = 0
    for position in range(open_brace, len(text)):
        if text[position] == "{":
            depth += 1
        elif text[position] == "}":
            depth -= 1
            if depth == 0:
                return declaration.start(), position + 1
    return declaration.start(), len(text)


def line_at(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def kind_at(text: str, offset: int) -> str:
    start = text.rfind("\n", 0, offset) + 1
    end = text.find("\n", offset)
    line = text[start:end if end >= 0 else len(text)]
    prefix = text[max(0, offset - 240):offset]
    if re.search(r"\bunwrap_or_else\s*\(", line):
        return "unwrap_or_else_noop"
    if re.search(r"\bthen_some\s*\(", prefix):
        return "then_some_unwrap_or_noop"
    if re.search(r"\bunwrap_or\s*\(", line):
        return "unwrap_or_noop"
    if re.search(r"\|[^|]*\|\s*(?:Message::)?Noop\b", line):
        return "closure_noop"
    return "direct_noop"


def statement_start(text: str, offset: int) -> int:
    """Start of the statement containing `offset`."""
    window_start = max(0, offset - MAX_SCAN)
    declarations = [match.start() for match in FUNCTION.finditer(text, window_start, offset)]
    if declarations:
        window_start = max(window_start, declarations[-1])
    window = text[window_start:offset]
    cut = max(window.rfind(delimiter) for delimiter in ";{}")
    return window_start + cut + 1


def call_spans(text: str) -> list[tuple[int, int, str]]:
    """Every widget and always-on handler call with its balanced argument span."""
    spans: list[tuple[int, int, str]] = []
    for call in CALL.finditer(text):
        name = call.group("name")
        if name not in WIDGETS and name not in HANDLERS:
            continue
        end = matching_paren(text, call.end() - 1)
        spans.append((call.end(), end, name))
    for call in MAYBE_HANDLERS.finditer(text):
        end = matching_paren(text, call.end() - 1)
        if re.fullmatch(r"\s*None\s*", text[call.end():end], re.S):
            spans.append((call.end(), end, "on_maybe_none"))
    return sorted(spans)


def enclosing_function(text: str, offset: int) -> str:
    declarations = list(FUNCTION.finditer(text, 0, offset))
    return declarations[-1].group("name") if declarations else ""


def owner_call(text: str, offset: int, spans: list[tuple[int, int, str]]) -> tuple[str, bool] | None:
    """Smallest enabled call whose argument contains this Noop."""
    disabled = any(start <= offset <= end and name == "on_maybe_none"
                   for start, end, name in spans)
    choices = [(end - start, start, end, name)
               for start, end, name in spans
               if start <= offset <= end and name != "on_maybe_none"]
    if not choices:
        return None
    _width, _start, _end, name = min(choices)
    return name, disabled


def helper_noops(text: str) -> dict[str, int]:
    """Noop-producing helper bodies, keyed by function name."""
    helpers: dict[str, int] = {}
    for declaration in FUNCTION.finditer(text):
        name = declaration.group("name")
        if not MESSAGE_HELPERS.search(name):
            continue
        span = function_span(text, declaration)
        if not span:
            continue
        start, end = span
        body = text[start:end]
        for match in NOOP.finditer(body):
            prefix = body[:match.start()]
            if FALLBACK.search(body[:match.start()]):
                helpers[name] = start + match.start()
    return helpers


def noop_can_escape(text: str, noop_offset: int, helper_noop_offsets: dict[str, int]) -> bool:
    """Conservatively decide whether a seen Noop is in an enabled handler."""
    start = statement_start(text, noop_offset)
    statement = text[start:noop_offset + 20]
    enclosing = [match.group("name") for match in FUNCTION.finditer(text, 0, start)]
    function_name = enclosing[-1] if enclosing else ""
    if LEGITIMATE_FUNCTION.search(function_name):
        return False
    if re.search(r"\bmouse_area\s*\(", text[max(0, start - 300):start]):
        return False
    spans = call_spans(text)
    call = owner_call(text, noop_offset, spans)
    if call and call[1]:
        return False
    if not call:
        for helper in helper_noop_offsets:
            span = function_span(text, next(
                declaration for declaration in FUNCTION.finditer(text)
                if declaration.group("name") == helper
            ))
            if span and span[0] <= noop_offset <= span[1] and FALLBACK.search(
                    text[span[0]:noop_offset]):
                return True
        return False
    before_noop = text[start:noop_offset]
    if FALLBACK.search(before_noop) or NOOP_CLOSURE.search(before_noop):
        return True
    helper_hits = [name for name in helper_noop_offsets
                   if re.search(r"\b" + re.escape(name) + r"\s*\(", statement)]
    if helper_hits:
        return True
    return re.search(r"\bMessage::Noop\s*\)", statement) is not None


def file_findings(path: pathlib.Path, text: str) -> dict[str, dict]:
    """Finding key -> site facts for one source file."""
    code = mask(text)
    relative = str(path.relative_to(REPO)) if path.is_relative_to(REPO) else path.name
    findings: dict[str, dict] = {}
    helpers = helper_noops(code)
    for name, offset in helpers.items():
        line = line_at(code, offset)
        key = f"{relative}:{line}:message_helper"
        findings[key] = {
            "file": relative,
            "line": line,
            "form": "message_helper",
            "kind": kind_at(code, offset),
            "widget": "icon_button",
            "handler": "enabled",
        }
    for match in NOOP.finditer(code):
        offset = match.start()
        if not noop_can_escape(code, offset, helpers):
            continue
        call = owner_call(code, offset, call_spans(code))
        if call:
            widget = call[0]
            form = "choice_widget" if widget in CHOICE_WIDGETS else "enabled_handler"
        else:
            widget = "icon_button"
            form = "message_helper"
        line = line_at(code, offset)
        findings[f"{relative}:{line}:{form}"] = {
            "file": relative,
            "line": line,
            "form": form,
            "kind": kind_at(code, offset),
            "widget": widget,
            "handler": "enabled",
        }
    return findings


def detect(root: pathlib.Path) -> dict[str, dict]:
    """All inert-control sites, keyed by exact file:line:form."""
    if root.is_file():
        paths = [root]
    else:
        paths = sorted(path for path in root.rglob("*.rs") if path.is_file())
    found: dict[str, dict] = {}
    for path in paths:
        found.update(file_findings(path, path.read_text(encoding="utf-8", errors="replace")))
    return found


def entry_key(entry: dict) -> str:
    form = entry.get("form", "enabled_handler")
    return f"{entry['file']}:{entry['line']}:{form}"


def read_inventory() -> list[dict]:
    if not INVENTORY.is_file():
        return []
    document = json.loads(INVENTORY.read_text(encoding="utf-8"))
    return list(document.get("sites", []))


def write_inventory(sites: list[dict]) -> None:
    INVENTORY.write_text(
        json.dumps({"version": 2, "sites": sites}, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def audit(root: pathlib.Path = VIEW, registered: list[dict] | None = None):
    """Return (new sites, stale entries). Both make the debt fail closed."""
    registered = read_inventory() if registered is None else registered
    found = detect(root)
    keys = {entry_key(entry) for entry in registered}
    new = [found[key] for key in sorted(found) if key not in keys]
    stale = [entry for entry in registered if entry_key(entry) not in found]
    return new, stale


SEVEN_SITES = (
    {
        "line": 214, "form": "enabled_handler", "widget": "icon_button",
        "kind": "unwrap_or_noop", "noop_line": 12,
        "source": "\n".join([
            "icon_button(",
            "    Icon::Target,",
            "    15.0,",
            "    group_info",
            "        .id",
            "        .clone()",
            "        .and_then(|id| {",
            "            state.outbound_probe_batch_message(OutboundProbeBatchScope::Collection(",
            "                id,",
            "            ))",
            "        })",
            "        .unwrap_or(Message::Noop),",
            ")",
        ]),
    },
    {
        "line": 244, "form": "enabled_handler", "widget": "icon_button",
        "kind": "unwrap_or_noop", "noop_line": 8,
        "source": "\n".join([
            "icon_button(",
            "    Icon::ArrowUp,",
            "    12.0,",
            "    group_info",
            "        .id",
            "        .clone()",
            "        .map(Message::MoveProxyGroupUp)",
            "        .unwrap_or(Message::Noop),",
            ")",
        ]),
    },
    {
        "line": 253, "form": "enabled_handler", "widget": "icon_button",
        "kind": "unwrap_or_noop", "noop_line": 8,
        "source": "\n".join([
            "icon_button(",
            "    Icon::ArrowDown,",
            "    12.0,",
            "    group_info",
            "        .id",
            "        .clone()",
            "        .map(Message::MoveProxyGroupDown)",
            "        .unwrap_or(Message::Noop),",
            ")",
        ]),
    },
    {
        "line": 263, "form": "enabled_handler", "widget": "icon_button",
        "kind": "unwrap_or_noop", "noop_line": 8,
        "source": "\n".join([
            "icon_button(",
            "    chevron_icon,",
            "    15.0,",
            "    group_info",
            "        .id",
            "        .clone()",
            "        .map(Message::ToggleOutboundGroupExpanded)",
            "        .unwrap_or(Message::Noop),",
            ")",
        ]),
    },
    {
        "line": 14, "form": "message_helper", "widget": "icon_button",
        "kind": "unwrap_or_noop", "noop_line": 5,
        "source": "\n".join([
            "fn favorite_message(state: &AppState, member_name: &str) -> Message {",
            "    member_view(state, member_name)",
            "        .and_then(|node| node.id.clone())",
            "        .map(Message::ToggleFavoriteOutbound)",
            "        .unwrap_or(Message::Noop)",
            "}",
        ]),
    },
    {
        "line": 243, "form": "choice_widget", "widget": "pick_list",
        "kind": "unwrap_or_noop", "noop_line": 7,
        "source": "\n".join([
            "pick_list(runtime_proxy_options, selected_runtime_proxy, |outbound| {",
            "    state",
            "        .prepare_runtime_outbound_selection_message(",
            "            &state.runtime.runtime_selected_group,",
            "            &outbound,",
            "        )",
            "        .unwrap_or(Message::Noop)",
            "},)",
        ]),
    },
    {
        "line": 286, "form": "choice_widget", "widget": "pick_list",
        "kind": "unwrap_or_noop", "noop_line": 7,
        "source": "\n".join([
            "pick_list(runtime_proxy_options, selected_runtime_proxy, |outbound| {",
            "    state",
            "        .prepare_runtime_outbound_selection_message(",
            "            &state.runtime.runtime_selected_group,",
            "            &outbound,",
            "        )",
            "        .unwrap_or(Message::Noop)",
            "},)",
        ]),
    },
)


def planted_sources() -> dict[str, str]:
    sources: dict[str, str] = {
        "typed.rs": "\n".join([
            "fn view(state: &AppState) -> Element<'_, Message> {",
            "    let live = icon_button(Icon::Target, 15.0, Message::ProbeAll);",
            "    let selector = pick_list(options, selected, |value| Message::Select(value));",
            "    row![live, selector].into()",
            "}",
        ]) + "\n",
        "disabled.rs": "\n".join([
            "fn view(state: &AppState) -> Element<'_, Message> {",
            "    button(text(\"unavailable\")).on_press_maybe(None).into();",
            "    let value = Message::Noop;",
            "    value",
            "}",
        ]) + "\n",
        "choice_closure.rs": "\n".join([
            "fn view(labels: &[String]) -> Element<'_, Message> {",
            "    segmented_control(labels, 0, |_| Message::Noop).into()",
            "}",
        ]) + "\n",
        "unwrap_or_else.rs": "\n".join([
            "fn view(state: &AppState) -> Element<'_, Message> {",
            "    icon_button(Icon::Pin, 13.0,",
            "        favorite_message(state).unwrap_or_else(|_| Message::Noop)).into()",
            "}",
        ]) + "\n",
        "then_some.rs": "\n".join([
            "fn view(state: &AppState) -> Element<'_, Message> {",
            "    icon_button(Icon::Pin, 13.0,",
            "        available.then_some(Message::Toggle).unwrap_or(Message::Noop)).into()",
            "}",
        ]) + "\n",
        "on_select.rs": "\n".join([
            "fn view(options: &[String]) -> Element<'_, Message> {",
            "    pick_list(options, None, Message::Select)",
            "        .on_select(|value| Message::Noop).into()",
            "}",
        ]) + "\n",
        "conditional.rs": "\n".join([
            "fn view(state: &AppState) -> Element<'_, Message> {",
            "    icon_button(Icon::Back, 13.0, if can_back {",
            "        Message::NavigateBack",
            "    } else {",
            "        Message::Noop",
            "    }).into()",
            "}",
        ]) + "\n",
        "legitimate.rs": "\n".join([
            "fn update_task(message: Message) -> Task<Message> {",
            "    Task::perform(async { do_work().await }, |_| Message::Noop)",
            "}",
            "fn modal_scrim() -> Element<'static, Message> {",
            "    mouse_area(backdrop).on_press(Message::Noop).into()",
            "}",
            "fn idle_message() { let value = Message::Noop; }",
        ]) + "\n",
        "comments.rs": "\n".join([
            "// .on_press(Message::Noop) is only a spelling in this comment",
            "let example = \"Message::Noop\";",
            "fn view() { let live = button(text(\"ok\")).on_press(Message::Apply); }",
        ]) + "\n",
    }
    for index, site in enumerate(SEVEN_SITES, start=1):
        padding = site["line"] - site["noop_line"]
        sources[f"current_{index}.rs"] = "\n" * padding + site["source"] + "\n"
    return sources


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary.name)
        for name, source in planted_sources().items():
            (self.root / name).write_text(source, encoding="utf-8")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_seven_current_sites_are_caught(self) -> None:
        for index, site in enumerate(SEVEN_SITES, start=1):
            with self.subTest(site=index):
                found = detect(self.root / f"current_{index}.rs")
                key = f"current_{index}.rs:{site['line']}:{site['form']}"
                self.assertEqual(len(found), 1, found)
                self.assertIn(key, found)
                self.assertEqual(found[key]["widget"], site["widget"])
                self.assertEqual(found[key]["kind"], site["kind"])
        self.assertEqual(len(SEVEN_SITES), 7)

    def test_typed_messages_are_not_flagged(self) -> None:
        self.assertEqual(detect(self.root / "typed.rs"), {})

    def test_disabled_control_is_not_flagged(self) -> None:
        self.assertEqual(detect(self.root / "disabled.rs"), {})

    def test_existing_noop_choice_is_still_caught(self) -> None:
        found = detect(self.root / "choice_closure.rs")
        self.assertTrue(any(site["widget"] == "segmented_control" for site in found.values()))

    def test_other_always_on_fallback_forms_are_caught(self) -> None:
        for name in ("unwrap_or_else.rs", "then_some.rs", "on_select.rs"):
            with self.subTest(form=name):
                self.assertTrue(detect(self.root / name), name)

    def test_conditional_and_legitimate_noop_uses_are_not_flagged(self) -> None:
        for name in ("conditional.rs", "legitimate.rs", "comments.rs"):
            with self.subTest(case=name):
                self.assertEqual(detect(self.root / name), {}, name)

    def test_live_findings_are_exactly_the_registered_inventory(self) -> None:
        found = detect(VIEW)
        registered = {entry_key(entry) for entry in read_inventory()}
        self.assertEqual(set(found), registered)
        self.assertEqual(len(registered), 7)

    def test_new_site_and_disappearing_registered_site_both_fail(self) -> None:
        global REPO
        original_repo = REPO
        registered = read_inventory()
        with tempfile.TemporaryDirectory() as directory:
            tree = pathlib.Path(directory)
            REPO = tree
            root = tree / "crates/infiltrator-iced/src"
            shutil.copytree(VIEW, root)
            try:
                (root / "view/planted.rs").write_text(
                    "fn view() { button(text(\"x\")).on_press(Message::Noop); }\n",
                    encoding="utf-8",
                )
                new, stale = audit(root, registered)
                self.assertEqual(len(new), 1)
                self.assertEqual(stale, [])
            finally:
                REPO = original_repo
        with tempfile.TemporaryDirectory() as directory:
            tree = pathlib.Path(directory)
            REPO = tree
            root = tree / "crates/infiltrator-iced/src"
            shutil.copytree(VIEW, root)
            try:
                proxies = root / "view/proxies.rs"
                proxies.write_text(proxies.read_text(encoding="utf-8").replace(
                    "unwrap_or(Message::Noop)", "unwrap_or(Message::Apply)", 1
                ), encoding="utf-8")
                new, stale = audit(root, registered)
                self.assertEqual(new, [])
                self.assertEqual(len(stale), 1)
            finally:
                REPO = original_repo


def self_test() -> int:
    stream = io.StringIO()
    with contextlib.redirect_stderr(stream):
        result = unittest.TextTestRunner(stream=stream, verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        )
    if not result.wasSuccessful():
        print(stream.getvalue(), end="")
        return 1
    print(stream.getvalue(), end="")
    print(f"iced dead-input guard self-test: PASS ({result.testsRun} planted tests, "
          f"{len(read_inventory())} live entries)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--update-inventory", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    registered = read_inventory()
    new, stale = audit(VIEW, registered)

    if args.update_inventory:
        if new:
            print(f"iced dead-input guard: refusing to register new inert controls: "
                  f"{[entry_key(site) for site in new]}")
            return 1
        found_keys = set(detect(VIEW))
        kept = [entry for entry in registered if entry_key(entry) in found_keys]
        write_inventory(kept)
        print(f"iced dead-input guard: inventory now {len(kept)} site(s), "
              f"dropped {len(registered) - len(kept)}")
        return 0

    for site in new:
        print(f"ICED-001 [{site['file']}:{site['line']}] unregistered {site['form']} "
              f"on {site['widget']} can emit Message::Noop")
    for entry in stale:
        print(f"ICED-001: inventory entry {entry_key(entry)} is stale - the inert control is gone, "
              "drop the entry")
    total = len(new) + len(stale)
    print(f"iced dead-input guard: findings={total} registered={len(registered)}")
    return 1 if total and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
