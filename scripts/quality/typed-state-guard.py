#!/usr/bin/env python3
"""Enforce typed multi-stage state at Rust/Kotlin boundaries.

Coverage is intentionally lexical and bounded. Production Rust under
``crates/**/src`` and Android Kotlin under ``android/**/src`` are tokenized
with nested comments and string/char literals removed. The guard examines:

* explicit phase carriers (``*State``, ``*Status``, ``*Lifecycle``, ``*Phase``
  and ``*Session`` records), but only when stored fields named as exclusive
  phases such as ``loading``, ``pending`` or ``failure`` are combined;
* Boolean request-kind/admission parameters and Boolean action outcomes on
  repository, port, adapter, ViewModel, lifecycle, service, host, bridge, IPC,
  owner, session, activity and application boundaries;
* braced ``if``/Rust ``match``/Kotlin ``when`` branches on stored Boolean phase
  fields when they write a phase field or call a transition operation.

Scalar setters (``set*``, ``update*`` and ``toggle*``) may carry real Boolean
domain values. Predicate queries and local ``clear``/``delete`` changed facts
are allowed; enum branches and return-only guards are not flag transitions.
This is a bounded syntax reader, not a compiler: macro expansion, type aliases,
cross-helper data flow and unbraced branches are outside its coverage. There is
no source suppression list; optional ``--path`` scopes one explicit incremental
check while the default remains a whole-repository scan. Inline Rust
``#[cfg(test)]`` modules and Android test source paths are excluded structurally.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[2]

BOUNDARY_PATH_WORDS = {
    "adapter", "adapters", "activity", "bridge", "client",
    "host", "ipc", "lifecycle", "owner", "port", "ports", "repository",
    "repositories", "service", "services", "viewmodel",
    "viewmodels",
}
BOUNDARY_SUFFIXES = (
    "Repository", "Port", "Adapter", "ViewModel", "Lifecycle", "Service",
    "Host", "Bridge", "Client", "Owner",
    "Activity", "Application", "Ipc", "IPC",
)
STATE_SUFFIXES = ("State", "Status", "Lifecycle", "Phase", "OperationState", "Session")

MUTATING_VERBS = {
    "accept", "activate", "add", "apply", "approve", "attach", "begin",
    "cancel", "clear", "close", "commit", "complete", "connect", "consume", "create",
    "deactivate", "delete", "detach", "disable", "disconnect", "enable",
    "execute", "flush", "grant", "handle", "install", "launch", "mutate",
    "open", "pause", "perform", "prepare", "process", "probe", "rebuild",
    "receive", "refresh", "release", "remove", "request", "restart", "resume",
    "revoke", "rollback", "run", "save", "select", "send", "set", "start",
    "stop", "store", "submit", "switch", "sync", "toggle", "uninstall",
    "update", "write",
}
PREDICATE_PREFIXES = (
    "is", "has", "can", "supports", "contains", "exists", "requires",
    "allows", "should", "was", "did",
)
ACTIVE_PHASE_FIELDS = {
    "attached", "busy", "connected", "initialized", "loading",
    "operation", "pending", "phase", "running", "starting", "status",
    "stopping",
}
TERMINAL_PHASE_FIELDS = {
    "cancelled", "canceled", "completed", "detached", "disconnected",
    "error", "failed", "failure", "loaded", "ready", "result", "stale",
    "stopped", "success", "succeeded", "unavailable",
}
PHASE_FIELDS = ACTIVE_PHASE_FIELDS | TERMINAL_PHASE_FIELDS
OPTION_PHASE_FIELDS = ACTIVE_PHASE_FIELDS | TERMINAL_PHASE_FIELDS
MIXED_ACTIVE_PHASE_FIELDS = {"busy", "loading", "pending", "starting", "stopping"}
TRANSIENT_OPERATION_WORDS = {
    "activating", "applying", "checking", "connecting", "creating", "deleting",
    "downloading", "executing", "exporting", "fetching", "importing", "installing",
    "loading", "migrating", "opening", "probing", "refreshing", "restarting",
    "restoring", "running", "saving", "starting", "stopping", "syncing", "testing",
    "updating", "uploading", "validating",
}
OPERATION_OPTION_SUFFIXES = {
    "cancel", "cancellation", "error", "failure", "operation", "pending",
    "phase", "progress", "request", "result", "task", "token",
}
VALUE_SETTER_VERBS = {"set", "update", "toggle"}
RESULT_FACT_VERBS = {"clear", "consume", "delete", "handle", "remove", "set", "toggle", "update"}
ASSIGNMENT_TOKENS = {"=", "+=", "-=", "*=", "/=", "%=", "|=", "&=", "^="}
OPEN_TO_CLOSE = {"(": ")", "[": "]", "{": "}"}


@dataclass(frozen=True)
class Token:
    value: str
    line: int
    kind: str = "symbol"


@dataclass(frozen=True)
class Field:
    name: str
    line: int
    is_bool: bool
    is_optional: bool
    is_status: bool


@dataclass
class Region:
    name: str
    kind: str
    start: int
    end: int
    line: int
    fields: list[Field]
    boundary: bool


@dataclass
class Function:
    name: str
    start: int
    signature_end: int
    body_open: int | None
    body_close: int | None
    params: list[tuple[str, list[Token], int]]
    return_type: list[Token]
    owner: Region | None
    line: int


@dataclass(frozen=True)
class Problem:
    line: int
    code: str
    detail: str


def _advance_line(source: str, line: int, start: int, end: int) -> int:
    return line + source.count("\n", start, end)


def tokenize(source: str) -> list[Token]:
    """Tokenize enough Rust/Kotlin syntax to preserve declaration/block scope."""
    tokens: list[Token] = []
    pos = 0
    line = 1
    size = len(source)
    operators = ("::", "->", "=>", "?.", "?:", "&&", "||", "==", "!=", "<=", ">=", "!!", "+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=")

    while pos < size:
        char = source[pos]
        if char.isspace():
            next_pos = pos + 1
            while next_pos < size and source[next_pos].isspace():
                next_pos += 1
            line = _advance_line(source, line, pos, next_pos)
            pos = next_pos
            continue

        if source.startswith("//", pos):
            end = source.find("\n", pos)
            pos = size if end < 0 else end
            continue

        if source.startswith("/*", pos):
            start = pos
            depth = 1
            pos += 2
            while pos < size and depth:
                if source.startswith("/*", pos):
                    depth += 1
                    pos += 2
                elif source.startswith("*/", pos):
                    depth -= 1
                    pos += 2
                else:
                    pos += 1
            line = _advance_line(source, line, start, pos)
            continue

        # Kotlin raw strings and Rust raw/byte-raw strings.
        raw_prefix = re.match(r"(?:br|rb|r)(?P<hashes>\#*)\"", source[pos:])
        if raw_prefix:
            start = pos
            hashes = raw_prefix.group("hashes")
            pos += raw_prefix.end()
            terminator = '"' + hashes
            end = source.find(terminator, pos)
            pos = size if end < 0 else end + len(terminator)
            line = _advance_line(source, line, start, pos)
            continue

        if source.startswith('"""', pos):
            start = pos
            pos += 3
            end = source.find('"""', pos)
            pos = size if end < 0 else end + 3
            line = _advance_line(source, line, start, pos)
            continue

        if char in ('"', "'"):
            # Rust lifetimes ('a, '_) are punctuation + identifier. A Rust or
            # Kotlin char literal closes immediately after one scalar (or one
            # escaped scalar); don't consume a lifetime because a later
            # literal happens to contain an apostrophe.
            if char == "'":
                if pos + 1 >= size:
                    tokens.append(Token("'", line))
                    pos += 1
                    continue
                if source[pos + 1] != "\\" and (pos + 2 >= size or source[pos + 2] != "'"):
                    tokens.append(Token("'", line))
                    pos += 1
                    continue
            start = pos
            quote = char
            pos += 1
            while pos < size:
                if source[pos] == "\\":
                    pos += min(2, size - pos)
                elif source[pos] == quote:
                    pos += 1
                    break
                else:
                    pos += 1
            line = _advance_line(source, line, start, pos)
            continue

        if char.isalpha() or char == "_":
            start = pos
            pos += 1
            while pos < size and (source[pos].isalnum() or source[pos] == "_"):
                pos += 1
            tokens.append(Token(source[start:pos], line, "ident"))
            continue

        if char.isdigit():
            start = pos
            pos += 1
            while pos < size and (source[pos].isalnum() or source[pos] in "_."):
                pos += 1
            tokens.append(Token(source[start:pos], line, "number"))
            continue

        operator = next((item for item in operators if source.startswith(item, pos)), None)
        if operator is not None:
            tokens.append(Token(operator, line))
            pos += len(operator)
        else:
            tokens.append(Token(char, line))
            pos += 1
    return tokens


def _matching(tokens: list[Token], start: int) -> int | None:
    opening = tokens[start].value
    closing = OPEN_TO_CLOSE.get(opening)
    if closing is None:
        return None
    depth = 1
    for index in range(start + 1, len(tokens)):
        value = tokens[index].value
        if value == opening:
            depth += 1
        elif value == closing:
            depth -= 1
            if depth == 0:
                return index
    return None


def _split_top_level(tokens: list[Token], delimiter: str = ",") -> list[list[Token]]:
    result: list[list[Token]] = []
    start = 0
    stack: list[str] = []
    angle_depth = 0
    for index, token in enumerate(tokens):
        value = token.value
        if value in OPEN_TO_CLOSE:
            stack.append(OPEN_TO_CLOSE[value])
        elif stack and value == stack[-1]:
            stack.pop()
        elif not stack and value == "<":
            angle_depth += 1
        elif not stack and value == ">" and angle_depth:
            angle_depth -= 1
        elif not stack and angle_depth == 0 and value == delimiter:
            result.append(tokens[start:index])
            start = index + 1
    result.append(tokens[start:])
    return result


def _first_top_level(tokens: list[Token], value: str) -> int | None:
    stack: list[str] = []
    angle_depth = 0
    for index, token in enumerate(tokens):
        current = token.value
        if current in OPEN_TO_CLOSE:
            stack.append(OPEN_TO_CLOSE[current])
        elif stack and current == stack[-1]:
            stack.pop()
        elif not stack and current == "<":
            angle_depth += 1
        elif not stack and current == ">" and angle_depth:
            angle_depth -= 1
        elif not stack and angle_depth == 0 and current == value:
            return index
    return None


def _snake(name: str) -> str:
    name = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", name)
    return re.sub(r"[^A-Za-z0-9]+", "_", name).strip("_").lower()


def _is_boundary_name(name: str) -> bool:
    return any(name.endswith(suffix) for suffix in BOUNDARY_SUFFIXES)


def _is_boundary_path(path: Path) -> bool:
    parts = [part.lower() for part in path.parts]
    if any(part in BOUNDARY_PATH_WORDS for part in parts):
        return True
    stem = path.stem.lower()
    if any(word in stem for word in ("viewmodel", "repository", "lifecycle", "service", "ipc", "bridge", "owner", "adapter", "activity")):
        return True
    return False


def _is_state_name(name: str) -> bool:
    return any(name.endswith(suffix) for suffix in STATE_SUFFIXES)


def _type_shape(tokens: list[Token]) -> tuple[bool, bool]:
    """Return (is_scalar_boolean, is_optional) for one field/parameter type."""
    values = [token.value for token in tokens]
    optional = "?" in values or any(value in {"Option", "Optional"} for value in values)
    if optional and "<" in values:
        start = values.index("<") + 1
        depth = 0
        inner: list[str] = []
        for value in values[start:]:
            if value == "<":
                depth += 1
            elif value == ">":
                if depth == 0:
                    break
                depth -= 1
            inner.append(value)
        values = inner
    values = [value for value in values if value not in {"?", "mut", "ref", "in", "out"}]
    return bool(len(values) == 1 and values[0] in {"bool", "Boolean"}), optional


def _status_field(name: str) -> bool:
    phase = _phase_name(name)
    return phase in PHASE_FIELDS or any(
        word in TRANSIENT_OPERATION_WORDS for word in phase.split("_")
    )


def _phase_name(name: str) -> str:
    words = _snake(name).split("_")
    if words and words[0] in {"is", "has", "can", "should", "needs", "was", "did"}:
        words = words[1:]
    return "_".join(words)


def _field(name: str, type_tokens: list[Token], line: int) -> Field:
    is_bool, optional = _type_shape(type_tokens)
    phase_name = _phase_name(name)
    is_status = _status_field(name) and (is_bool or phase_name in OPTION_PHASE_FIELDS)
    return Field(name, line, is_bool, optional, is_status)


def _operation_stem(name: str) -> str:
    words = _phase_name(name).split("_")
    if not words:
        return ""
    first = words[0]
    irregular = {"applying": "apply", "dying": "die", "lying": "lie", "running": "run", "saving": "save"}
    if first in irregular:
        return irregular[first]
    if first.endswith("ing") and len(first) > 4:
        return first[:-3]
    return first


def _operation_option_stem(name: str) -> str | None:
    words = _phase_name(name).split("_")
    if len(words) > 1 and words[-1] in OPERATION_OPTION_SUFFIXES:
        return words[0]
    return None


def _named_fields(tokens: list[Token], language: str) -> list[Field]:
    fields: list[Field] = []
    for chunk in _split_top_level(tokens):
        if not chunk:
            continue
        if language == "rust":
            colon = _first_top_level(chunk, ":")
            if colon is None:
                continue
            before = [token for token in chunk[:colon] if token.kind == "ident" and token.value not in {"pub", "mut", "crate", "self", "super"}]
            if not before:
                continue
            name_token = before[-1]
            fields.append(_field(name_token.value, chunk[colon + 1 :], name_token.line))
        else:
            name_index = next(
                (index for index, token in enumerate(chunk) if token.value in {"val", "var"}),
                None,
            )
            if name_index is None or name_index + 1 >= len(chunk):
                continue
            name_token = chunk[name_index + 1]
            colon = _first_top_level(chunk[name_index + 2 :], ":")
            if colon is None:
                continue
            colon += name_index + 2
            type_tokens = chunk[colon + 1 :]
            default = _first_top_level(type_tokens, "=")
            if default is not None:
                type_tokens = type_tokens[:default]
            fields.append(_field(name_token.value, type_tokens, name_token.line))
    return fields


def _property_fields(tokens: list[Token], open_index: int, close_index: int) -> list[Field]:
    """Read direct Kotlin val/var properties, skipping method/nested bodies."""
    fields: list[Field] = []
    index = open_index + 1
    while index < close_index:
        value = tokens[index].value
        if value in {"val", "var"}:
            line = tokens[index].line
            end = index + 1
            while end < close_index and tokens[end].line == line and tokens[end].value not in {"=", "by", ";"}:
                end += 1
            if not _has_kotlin_getter(tokens, index, close_index):
                fields.extend(_named_fields(tokens[index:end], "kotlin"))
            index = end
            continue
        if value == "{" and (end := _matching(tokens, index)) is not None:
            index = end + 1
        else:
            index += 1
    return fields


def _has_kotlin_getter(tokens: list[Token], start: int, end: int) -> bool:
    """Recognize computed getter syntax so derived projections are not fields."""
    line = tokens[start].line
    index = start + 1
    stack: list[str] = []
    while index < end:
        value = tokens[index].value
        if not stack:
            if value == "get" and index + 1 < end and tokens[index + 1].value == "(":
                return True
            if value == "=" or value == ";":
                return False
            if value in {"val", "var", "fun", "class", "interface", "object"} and tokens[index].line > line:
                return False
        if value in {"(", "[", "{"}:
            stack.append(OPEN_TO_CLOSE[value])
        elif stack and value == stack[-1]:
            stack.pop()
        index += 1
    return False


def _find_header_body(tokens: list[Token], start: int) -> tuple[int | None, int | None]:
    stack: list[str] = []
    angle_depth = 0
    for index in range(start + 1, len(tokens)):
        value = tokens[index].value
        if value == ";" and not stack and angle_depth == 0:
            return None, None
        if value == "{" and not stack and angle_depth == 0:
            return index, _matching(tokens, index)
        if value in {"(", "["}:
            stack.append(OPEN_TO_CLOSE[value])
        elif stack and value == stack[-1]:
            stack.pop()
        elif not stack and value == "<":
            angle_depth += 1
        elif not stack and value == ">" and angle_depth:
            angle_depth -= 1
    return None, None


def _kotlin_type_header(tokens: list[Token], start: int, name_line: int) -> tuple[int | None, int | None, int | None]:
    """Find a Kotlin type constructor/body without crossing a later declaration."""
    stack: list[str] = []
    angle_depth = 0
    constructor_open = None
    declarations = {"class", "interface", "object", "fun", "val", "var"}
    for index in range(start + 1, len(tokens)):
        value = tokens[index].value
        if not stack and angle_depth == 0:
            if value == "{" and (body_close := _matching(tokens, index)) is not None:
                return index, body_close, constructor_open
            if value == "(" and constructor_open is None:
                constructor_open = index
            elif value in declarations and tokens[index].line > name_line:
                return None, None, constructor_open
            elif value == ";":
                return None, None, constructor_open
        if value in {"(", "["}:
            stack.append(OPEN_TO_CLOSE[value])
        elif stack and value == stack[-1]:
            stack.pop()
        elif not stack and value == "<":
            angle_depth += 1
        elif not stack and value == ">" and angle_depth:
            angle_depth -= 1
    return None, None, constructor_open


def _function_parens(tokens: list[Token], name_index: int, language: str) -> int | None:
    """Locate only a declaration's parameter list, not a later function's."""
    angle_depth = 0
    for index in range(name_index + 1, len(tokens)):
        value = tokens[index].value
        if value == "<":
            angle_depth += 1
        elif value == ">" and angle_depth:
            angle_depth -= 1
        elif value == "(" and angle_depth == 0:
            return index
        elif angle_depth == 0 and value in {"{", "=", ";", "fn", "fun", "class", "impl"}:
            return None
    return None


def _parse_regions(tokens: list[Token], language: str, boundary_path: bool) -> list[Region]:
    regions: list[Region] = []
    keywords = {"struct", "enum", "trait", "class", "interface", "object"}
    if language == "kotlin":
        keywords = {"class", "interface", "object"}
    index = 0
    while index < len(tokens):
        if tokens[index].value == "impl" and language == "rust":
            body_open, body_close = _find_header_body(tokens, index)
            if body_open is not None and body_close is not None:
                head = tokens[index + 1 : body_open]
                for_token = next((i for i, token in enumerate(head) if token.value == "for"), None)
                owner_tokens = head[for_token + 1 :] if for_token is not None else head
                owner_name = next((token.value for token in owner_tokens if token.kind == "ident"), "")
                trait_name = next((token.value for token in head if token.kind == "ident"), "")
                name = owner_name or trait_name or "impl"
                boundary = boundary_path or _is_boundary_name(owner_name) or _is_boundary_name(trait_name)
                regions.append(Region(name, "impl", index, body_close, tokens[index].line, [], boundary))
            index += 1
            continue

        if tokens[index].value not in keywords:
            index += 1
            continue
        kind = tokens[index].value
        name_index = index + 1
        if name_index >= len(tokens) or tokens[name_index].kind != "ident":
            index += 1
            continue
        name = tokens[name_index].value
        if language == "kotlin":
            body_open, body_close, constructor_open = _kotlin_type_header(
                tokens, name_index, tokens[name_index].line
            )
        else:
            body_open, body_close = _find_header_body(tokens, name_index)
            constructor_open = None
        fields: list[Field] = []
        constructor_close = None
        if language == "kotlin":
            if constructor_open is not None:
                constructor_close = _matching(tokens, constructor_open)
        if body_open is None or body_close is None:
            if language == "kotlin" and kind == "class" and constructor_open is not None and constructor_close is not None:
                fields.extend(_named_fields(tokens[constructor_open + 1 : constructor_close], language))
                boundary = boundary_path or _is_boundary_name(name)
                regions.append(Region(name, kind, index, constructor_close, tokens[index].line, fields, boundary))
                index = constructor_close + 1
                continue
            index += 1
            continue
        if language == "rust" and kind == "struct":
            fields = _named_fields(tokens[body_open + 1 : body_close], language)
        elif language == "kotlin":
            if constructor_open is not None and constructor_close is not None and constructor_close < body_open:
                fields.extend(_named_fields(tokens[constructor_open + 1 : constructor_close], language))
            fields.extend(_property_fields(tokens, body_open, body_close))
        boundary = boundary_path or _is_boundary_name(name)
        regions.append(Region(name, kind, index, body_close, tokens[index].line, fields, boundary))
        index = name_index + 1
    return regions


def _signature_end(tokens: list[Token], start: int, language: str) -> int:
    stack: list[str] = []
    angle_depth = 0
    for index in range(start, len(tokens)):
        value = tokens[index].value
        if value in OPEN_TO_CLOSE:
            if value == "{" and not stack and angle_depth == 0:
                return index
            stack.append(OPEN_TO_CLOSE[value])
        elif stack and value == stack[-1]:
            stack.pop()
        elif not stack and value == "<":
            angle_depth += 1
        elif not stack and value == ">" and angle_depth:
            angle_depth -= 1
        elif not stack and angle_depth == 0 and value in {";", "="}:
            return index
    return len(tokens)


def _parse_functions(tokens: list[Token], language: str, regions: list[Region]) -> list[Function]:
    keyword = "fn" if language == "rust" else "fun"
    functions: list[Function] = []
    for index, token in enumerate(tokens):
        if token.value != keyword or index + 1 >= len(tokens):
            continue
        name_index = index + 1
        if language == "kotlin":
            # Extension functions may have a receiver before the final name.
            open_index = _function_parens(tokens, name_index, language)
            if open_index is None:
                continue
            candidates = []
            angle_depth = 0
            for candidate in range(name_index, open_index):
                value = tokens[candidate].value
                if value == "<":
                    angle_depth += 1
                elif value == ">" and angle_depth:
                    angle_depth -= 1
                elif angle_depth == 0 and tokens[candidate].kind == "ident":
                    candidates.append(candidate)
            if not candidates:
                continue
            name_index = candidates[-1]
        elif tokens[name_index].kind != "ident":
            continue
        else:
            open_index = _function_parens(tokens, name_index, language)
        if open_index is None:
            continue
        close_index = _matching(tokens, open_index)
        if close_index is None:
            continue
        sig_end = _signature_end(tokens, close_index + 1, language)
        body_open = sig_end if sig_end < len(tokens) and tokens[sig_end].value == "{" else None
        body_close = _matching(tokens, body_open) if body_open is not None else None
        params: list[tuple[str, list[Token], int]] = []
        for chunk in _split_top_level(tokens[open_index + 1 : close_index]):
            colon = _first_top_level(chunk, ":")
            if colon is None:
                continue
            name_candidates = [item for item in chunk[:colon] if item.kind == "ident" and item.value not in {"mut", "ref", "self", "val", "var"}]
            if not name_candidates:
                continue
            type_tokens = chunk[colon + 1 :]
            default = _first_top_level(type_tokens, "=")
            if default is not None:
                type_tokens = type_tokens[:default]
            params.append((name_candidates[-1].value, type_tokens, name_candidates[-1].line))

        return_type: list[Token] = []
        if language == "rust":
            arrow = next((i for i in range(close_index + 1, sig_end) if tokens[i].value == "->"), None)
            if arrow is not None:
                return_type = tokens[arrow + 1 : sig_end]
        else:
            colon = next((i for i in range(close_index + 1, sig_end) if tokens[i].value == ":"), None)
            if colon is not None:
                return_type = tokens[colon + 1 : sig_end]

        owner = None
        for region in regions:
            if region.start < index < region.end and (owner is None or region.start > owner.start):
                owner = region
        functions.append(
            Function(
                tokens[name_index].value,
                index,
                sig_end,
                body_open,
                body_close,
                params,
                return_type,
                owner,
                token.line,
            )
        )
    return functions


def _is_boolean_parameter(tokens: list[Token]) -> bool:
    is_bool, _ = _type_shape(tokens)
    return is_bool


def _is_boolean_outcome(tokens: list[Token]) -> bool:
    values = [token.value for token in tokens if token.value not in {"?", "&", "mut", "dyn", "impl"}]
    if len(values) == 1 and values[0] in {"bool", "Boolean"}:
        return True
    if len(values) >= 2 and values[0] == "(" and values[-1] == ")":
        parts = _split_top_level([Token(value, 0) for value in values[1:-1]])
        return any(_is_boolean_outcome(part) for part in parts)
    wrapper_names = {"Result", "CallResult", "Outcome", "Either", "Option", "StateFlow", "Flow", "Deferred", "Pair", "Triple"}
    if values and "<" in values:
        try:
            opening = values.index("<")
            wrapper = next(value for value in reversed(values[:opening]) if value.isidentifier())
            closing = len(values) - 1 - values[::-1].index(">")
            if wrapper not in wrapper_names or closing <= opening:
                return False
            args = _split_top_level([Token(value, 0) for value in values[opening + 1 : closing]])
            return bool(args and _is_boolean_outcome(args[0]))
        except (ValueError, IndexError):
            return False
    return False


def _name_words(name: str) -> list[str]:
    return _snake(name).split("_")


def _is_predicate(name: str) -> bool:
    words = _name_words(name)
    return bool(words and (words[0] in PREDICATE_PREFIXES or words[-1] in {"requested", "available", "enabled", "supported"}))


def _is_mutating(name: str) -> bool:
    words = _name_words(name)
    return bool(words and words[0] in MUTATING_VERBS)


def _is_parameter_operation(name: str) -> bool:
    words = _name_words(name)
    return bool(words and words[0] in MUTATING_VERBS - VALUE_SETTER_VERBS)


def _is_result_operation(name: str) -> bool:
    words = _name_words(name)
    return not _is_predicate(name) and any(
        word in MUTATING_VERBS - RESULT_FACT_VERBS for word in words
    )


def _returns_state_transition(function: Function, tokens: list[Token]) -> bool:
    """Catch Boolean lifecycle admission even when a transition has a noun name."""
    if function.body_open is None or function.body_close is None:
        return False
    body = tokens[function.body_open + 1 : function.body_close]
    returns_boolean_literal = any(
        body[index].value == "return"
        and index + 1 < len(body)
        and body[index + 1].value in {"true", "false"}
        for index in range(len(body))
    )
    updates_state = any(
        token.value in {"update", "transition"}
        and index + 1 < len(body)
        and body[index + 1].value == "("
        for index, token in enumerate(body)
    )
    return returns_boolean_literal and updates_state


def _conditional_spans(tokens: list[Token], start: int, end: int) -> list[tuple[int, int, int, int]]:
    """Return (condition start/end, body start/end) for braced conditionals."""
    spans: list[tuple[int, int, int, int]] = []
    for index in range(start, end):
        if tokens[index].value not in {"if", "match", "when"}:
            continue
        condition_start = index + 1
        if condition_start < end and tokens[condition_start].value == "(" and (condition_close := _matching(tokens, condition_start)) is not None:
            condition_end = condition_close
            body_open = condition_close + 1
            while body_open < end and tokens[body_open].value not in {"{", ";", "="}:
                body_open += 1
        else:
            stack: list[str] = []
            body_open = condition_start
            while body_open < end:
                value = tokens[body_open].value
                if value == "{" and not stack:
                    break
                if value in {"(", "["}:
                    stack.append(OPEN_TO_CLOSE[value])
                elif stack and value == stack[-1]:
                    stack.pop()
                body_open += 1
            condition_end = body_open
        if body_open >= end or tokens[body_open].value != "{":
            continue
        body_close = _matching(tokens, body_open)
        if body_close is None or body_close > end:
            continue
        body_end = body_close
        if tokens[index].value == "if" and body_close + 1 < end and tokens[body_close + 1].value == "else":
            else_start = body_close + 2
            if else_start < end and tokens[else_start].value == "{" and (else_close := _matching(tokens, else_start)) is not None:
                body_end = else_close
        spans.append((condition_start, condition_end, body_open + 1, body_end))
    return spans


def _read_control_field(condition: list[Token], field_names: set[str], owner_names: set[str]) -> set[str]:
    found: set[str] = set()
    for index, token in enumerate(condition):
        if token.value not in field_names:
            continue
        previous = condition[index - 1].value if index else ""
        if previous in {".", "?."} or token.value in owner_names:
            found.add(token.value)
    return found


def _written_control_field(body: list[Token], field_names: set[str]) -> set[str]:
    written: set[str] = set()
    for index, token in enumerate(body):
        if token.value in field_names and index + 1 < len(body) and body[index + 1].value in ASSIGNMENT_TOKENS:
            written.add(token.value)
        if token.value in field_names and index + 1 < len(body) and body[index + 1].value == "(" and token.value.startswith("set"):
            written.add(token.value)
    return written


def _has_effectful_branch(body: list[Token]) -> bool:
    for index, token in enumerate(body[:-1]):
        if token.kind == "ident" and token.value in MUTATING_VERBS and body[index + 1].value == "(":
            return True
    return False


def _without_cfg_test_modules(tokens: list[Token]) -> list[Token]:
    """Drop Rust items under an exact ``#[cfg(test)] mod`` attribute."""
    kept: list[Token] = []
    index = 0
    while index < len(tokens):
        if tokens[index].value != "#" or index + 1 >= len(tokens) or tokens[index + 1].value != "[":
            kept.append(tokens[index])
            index += 1
            continue
        attribute_close = _matching(tokens, index + 1)
        if attribute_close is None:
            kept.append(tokens[index])
            index += 1
            continue
        attribute = [token.value for token in tokens[index + 2 : attribute_close]]
        is_cfg_test = any(
            attribute[offset : offset + 4] == ["cfg", "(", "test", ")"]
            for offset in range(max(0, len(attribute) - 3))
        )
        if not is_cfg_test:
            kept.append(tokens[index])
            index += 1
            continue
        declaration_end = attribute_close + 1
        while declaration_end < len(tokens) and tokens[declaration_end].value not in {"{", ";"}:
            declaration_end += 1
        header = [token.value for token in tokens[attribute_close + 1 : declaration_end]]
        if "mod" not in header or declaration_end >= len(tokens):
            kept.append(tokens[index])
            index += 1
            continue
        if tokens[declaration_end].value == ";":
            index = declaration_end + 1
        else:
            body_close = _matching(tokens, declaration_end)
            index = len(tokens) if body_close is None else body_close + 1
    return kept


def analyze_source(path: Path, source: str) -> list[Problem]:
    language = "rust" if path.suffix == ".rs" else "kotlin" if path.suffix == ".kt" else ""
    if not language:
        return []
    tokens = _without_cfg_test_modules(tokenize(source)) if path.suffix == ".rs" else tokenize(source)
    boundary_path = _is_boundary_path(path)
    regions = _parse_regions(tokens, language, boundary_path)
    functions = _parse_functions(tokens, language, regions)
    problems: list[Problem] = []

    state_regions = [region for region in regions if _is_state_name(region.name) and region.fields]
    all_control_fields: set[str] = set()
    for region in regions:
        if not region.boundary and not _is_state_name(region.name):
            continue
        all_control_fields.update(
            field.name for field in region.fields
            if field.is_status and field.is_bool and not field.is_optional
        )
    state_control_owners = {
        region.name
        for region in state_regions
        if any(field.is_status and field.is_bool and not field.is_optional for field in region.fields)
    }
    for region in state_regions:
        status_bools = [field for field in region.fields if field.is_status and field.is_bool and not field.is_optional]
        option_stems = {
            stem
            for field in region.fields
            if field.is_optional
            if (stem := _operation_option_stem(field.name)) is not None
        }
        operation_bools = [
            field
            for field in status_bools
            if _phase_name(field.name).split("_")[0] in TRANSIENT_OPERATION_WORDS
        ]
        related_options = {
            _operation_stem(field.name)
            for field in operation_bools
            if _operation_stem(field.name) in option_stems
        }
        status_options = [
            field
            for field in region.fields
            if field.is_optional
            and (field.is_status or _operation_option_stem(field.name) in related_options)
        ]
        bool_phases = {_phase_name(field.name) for field in status_bools}
        optional_phases = {_phase_name(field.name) for field in status_options}
        active_phases = bool_phases & MIXED_ACTIVE_PHASE_FIELDS
        terminal_phases = bool_phases & TERMINAL_PHASE_FIELDS
        active_options = optional_phases & ACTIVE_PHASE_FIELDS
        terminal_options = optional_phases & TERMINAL_PHASE_FIELDS
        encoded = (
            len(status_bools) >= 2
            or bool(active_phases and terminal_options)
            or bool(active_options and terminal_options)
            or bool(related_options)
        )
        if encoded:
            names = [field.name for field in region.fields if field.is_status and (field.is_bool or field.is_optional)]
            problems.append(
                Problem(
                    region.line,
                    "MULTI_FIELD_STATE",
                    f"{region.name} encodes exclusive stages across bool/Option fields {', '.join(names)}; use one enum/sealed state",
                )
            )
        if operation_bools:
            names = ", ".join(field.name for field in operation_bools)
            problems.append(
                Problem(
                    operation_bools[0].line,
                    "UNTYPED_OPERATION_STATE",
                    f"{region.name} stores async operation phase in Boolean fields {names}; use one explicit enum state",
                )
            )

    for function in functions:
        in_boundary = boundary_path or (function.owner is not None and function.owner.boundary)
        in_state_scope = function.owner is not None and function.owner.name in state_control_owners
        if in_boundary and _is_parameter_operation(function.name):
            for parameter_name, parameter_type, line in function.params:
                if _is_boolean_parameter(parameter_type):
                    problems.append(
                        Problem(line, "BOOL_OPERATION_PARAMETER", f"{function.name} uses Boolean parameter `{parameter_name}` for operation kind/state; use a typed enum/input")
                    )
        if in_boundary and _is_boolean_outcome(function.return_type) and (
            _is_result_operation(function.name) or _returns_state_transition(function, tokens)
        ):
            problems.append(
                Problem(function.line, "BOOL_OPERATION_RESULT", f"{function.name} returns Boolean as an operation outcome; return a typed receipt/result")
            )

        if not (in_boundary or in_state_scope) or function.body_open is None or function.body_close is None:
            continue
        body_start = function.body_open + 1
        body_end = function.body_close
        owner_fields = set()
        if function.owner is not None:
            owner_fields.update(
                field.name for field in function.owner.fields
                if field.is_status and field.is_bool and not field.is_optional
            )
        for condition_start, condition_end, branch_start, branch_end in _conditional_spans(tokens, body_start, body_end):
            condition = tokens[condition_start:condition_end]
            branch = tokens[branch_start:branch_end]
            reads = _read_control_field(condition, all_control_fields, owner_fields)
            if reads and (_written_control_field(branch, all_control_fields) or _has_effectful_branch(branch)):
                problems.append(
                    Problem(
                        tokens[condition_start - 1].line if condition_start else function.line,
                        "FLAG_TRANSITION",
                        f"{function.name} branches on status flag(s) {', '.join(sorted(reads))} before transition/operation; use exhaustive enum/sealed matching",
                    )
                )
                break
    return problems


def _excluded_source(path: Path) -> bool:
    parts = {part.lower() for part in path.parts}
    stem = path.stem.lower()
    return bool(parts & {"test", "tests", "testing", "benches", "examples"}) or stem in {"test", "tests"} or stem.startswith("test_") or stem.endswith(("_test", "_tests"))


def source_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for base in (root / "crates", root / "android"):
        if not base.exists():
            continue
        for suffix in ("*.rs", "*.kt"):
            files.extend(path for path in base.rglob(suffix) if "src" in path.parts and not _excluded_source(path))
    return sorted(set(files))


def scan_repository(root: Path = ROOT) -> list[tuple[Path, Problem]]:
    findings: list[tuple[Path, Problem]] = []
    for path in source_files(root):
        source = path.read_text(encoding="utf-8")
        for problem in analyze_source(path.relative_to(root), source):
            findings.append((path, problem))
    return findings


def filter_findings(
    findings: list[tuple[Path, Problem]],
    requested_paths: list[Path],
    root: Path = ROOT,
) -> list[tuple[Path, Problem]]:
    if not requested_paths:
        return findings

    repository = root.resolve()
    sources = {path.resolve() for path in source_files(repository)}
    selected: set[Path] = set()
    for requested in requested_paths:
        scope = (repository / requested).resolve()
        try:
            scope.relative_to(repository)
        except ValueError as error:
            raise ValueError(f"typed-state scope escapes repository: {requested}") from error
        if not scope.exists():
            raise ValueError(f"typed-state scope does not exist: {requested}")
        matches = {
            source
            for source in sources
            if source == scope or source.is_relative_to(scope)
        }
        if not matches:
            raise ValueError(f"typed-state scope contains no scanned Rust/Kotlin source: {requested}")
        selected.update(matches)

    return [(path, problem) for path, problem in findings if path.resolve() in selected]


def self_test() -> int:
    positives = {
        "rust_binary_fact.rs": """
            struct VpnSettingsState { enabled: bool, ipv6: bool }
            trait VpnLifecycleRepository { fn set_enabled(&self, enabled: bool); fn is_enabled(&self) -> bool; }
        """,
        "kotlin_enum_state.kt": """
            class CoreLifecycleViewModel {
                sealed interface State { data object Loading : State; data class Ready(val revision: Long) : State }
                fun start(): State = State.Ready(1)
                fun isRunning(): Boolean = true
            }
        """,
        "kotlin_unrelated.kt": """
            class Formatter { fun format(enabled: Boolean): String = enabled.toString() }
        """,
    }
    for name, source in positives.items():
        if analyze_source(Path(name), source):
            raise AssertionError(f"positive sample {name} rejected: {analyze_source(Path(name), source)}")

    negatives = {
        "rust_repo_state.rs": (
            "struct VpnServiceState { is_loading: bool, is_running: bool, error: Option<String> }",
            "MULTI_FIELD_STATE",
        ),
        "kotlin_viewmodel_state.kt": (
            "data class ProxyViewModelState(val isLoading: Boolean, val error: String?, val saved: Boolean)",
            "MULTI_FIELD_STATE",
        ),
        "rust_lifecycle_parameter.rs": (
            "trait VpnLifecycleRepository { fn start(&self, resume: bool); }",
            "BOOL_OPERATION_PARAMETER",
        ),
        "kotlin_ipc_result.kt": (
            "class OwnerIpcClient { fun attach(): Boolean = true }",
            "BOOL_OPERATION_RESULT",
        ),
        "kotlin_flag_transition.kt": (
            "class VpnLifecycleService { var isRunning: Boolean = false; fun stop() { if (isRunning) { isRunning = false; release() } }; fun release() {} }",
            "FLAG_TRANSITION",
        ),
    }
    for name, (source, expected) in negatives.items():
        codes = {problem.code for problem in analyze_source(Path(name), source)}
        if expected not in codes:
            raise AssertionError(f"negative sample {name} missed {expected}: {codes}")

    adversarial = '''
        // class FakeRepository { fun stop(force: Boolean): Boolean = true }
        class VpnService {
            val label = "if (isLoading) { isLoading = false } fun attach(retry: Boolean)"
            /* struct Status { running: bool, stopped: bool } */
            fun isRunning(): Boolean = true
        }
    '''
    if analyze_source(Path("VpnService.kt"), adversarial):
        raise AssertionError("comments and literals must not create state findings")

    suppression = '''
        @Suppress("TypedState")
        class OwnerLifecycleRepository { fun pause(force: Boolean) {} }
    '''
    if "BOOL_OPERATION_PARAMETER" not in {item.code for item in analyze_source(Path("OwnerLifecycleRepository.kt"), suppression)}:
        raise AssertionError("annotations must not suppress typed-state violations")

    print("typed-state guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument(
        "--path",
        action="append",
        type=Path,
        default=[],
        help="scope this incremental check to a repository-relative Rust/Kotlin source file or directory; repeat as needed",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    try:
        findings = filter_findings(scan_repository(), args.path)
    except ValueError as error:
        parser.error(str(error))
    for path, problem in findings:
        location = path.relative_to(ROOT)
        print(f"VIOLATION [{args.mode}]: {location}:{problem.line}: {problem.code}: {problem.detail}", file=sys.stderr)
    print(f"typed-state guard: findings={len(findings)}")
    return 1 if findings and args.mode == "enforce" else 0


if __name__ == "__main__":
    raise SystemExit(main())
