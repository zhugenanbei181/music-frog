#!/usr/bin/env python3
"""Guard the Android architecture red lines in the Kotlin production tree.

BANDROID-044.4 forbids seven production patterns in the hand written Android
Kotlin sources:

* ``ARCH001`` a production readiness sleep (``Thread.sleep``);
* ``ARCH002`` a blocking wait on the Android main thread;
* ``ARCH003`` an unbounded ``GlobalScope`` coroutine;
* ``ARCH004`` an empty ``catch`` block;
* ``ARCH005`` a swallowed ``CancellationException``;
* ``ARCH006`` a not-null assertion (``!!``);
* ``ARCH007`` a multi-state Boolean wire, that is one type that carries two
  or more phase-like Boolean properties instead of a typed phase.

A narrow exception is allowed only when the offending line carries a local
explanation *and* the subject of the violation is covered by a behaviour
regression test:

    // ARCH-REDLINE: the typed owner wait directive is bounded and runs on the VPN worker thread
    Thread.sleep(directive.millis)

The guard is a bounded lexical reader. It masks comments and string literals,
tracks brace blocks with their declaration headers, and never resolves types,
so it reports a pattern, not a data-flow proof. It scans the production tree
only: the generated UniFFI binding and every test source set are outside its
scope, exactly like ``scripts/quality/kt-line-guard.py`` treats them.

Usage:
    python3 scripts/quality/android-architecture-redline-guard.py [--mode report|enforce]
    python3 scripts/quality/android-architecture-redline-guard.py --self-test

Exit status is 0 when no violation is found (or in report mode), 1 when
violations exist in enforce mode.
"""

from __future__ import annotations

import argparse
import dataclasses
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

#: The hand written Android production tree.
PRODUCTION_ROOTS = ("android/app/src/main/java",)
#: Generated code is not hand written and is never a reviewable contract.
GENERATED_MARKERS = ("infiltrator_android",)
#: The test source sets that can carry a behaviour regression.
TEST_ROOTS = (
    "android/app/src/test/java",
    "android/app/src/androidTest/java",
    "android/app/src/androidTestCompose/java",
)
#: The presentation layer runs on the Android main thread.
PRESENTATION_MARKERS = ("/ui/",)

#: Android callbacks that the framework runs on the main thread.
MAIN_THREAD_CALLBACKS = frozenset(
    {
        "onCreate",
        "onStart",
        "onResume",
        "onPause",
        "onStop",
        "onDestroy",
        "onBind",
        "onUnbind",
        "onRebind",
        "onReceive",
        "onNewIntent",
        "onActivityResult",
        "onRequestPermissionsResult",
        "onSaveInstanceState",
        "onRestoreInstanceState",
        "onTrimMemory",
        "onConfigurationChanged",
    }
)
#: Coroutine and thread builders that move the body off the calling thread.
DEFERRED_BUILDERS = (
    "launch",
    "async",
    "withContext",
    "Thread",
    "submit",
    "execute",
    "GlobalScope",
)
#: Boolean property names that describe one phase of a multi-stage flow.
PHASE_WORDS = frozenset(
    {
        "active",
        "attached",
        "available",
        "busy",
        "cancelled",
        "canceled",
        "cleared",
        "clearing",
        "closed",
        "closing",
        "completed",
        "connected",
        "detached",
        "disabled",
        "disconnected",
        "dirty",
        "empty",
        "enabled",
        "error",
        "failed",
        "failure",
        "initialized",
        "loaded",
        "loading",
        "offline",
        "online",
        "opened",
        "opening",
        "operation",
        "pending",
        "phase",
        "present",
        "ready",
        "refreshing",
        "result",
        "running",
        "saving",
        "saved",
        "selected",
        "starting",
        "status",
        "stale",
        "stopped",
        "stopping",
        "succeeded",
        "success",
        "terminating",
        "unavailable",
        "unselected",
    }
)
#: The prefix and suffix that a phase-like Boolean name may carry.
PHASE_AFFIXES = ("is", "has", "can", "should", "was", "did", "needs")

WAIVER_MARKER = "ARCH-REDLINE:"
MINIMUM_REASON_LENGTH = 20

RULE_TITLES = {
    "ARCH001": "production readiness sleep",
    "ARCH002": "main thread blocking wait",
    "ARCH003": "unbounded GlobalScope coroutine",
    "ARCH004": "empty catch block",
    "ARCH005": "swallowed cancellation",
    "ARCH006": "not-null assertion",
    "ARCH007": "multi-state Boolean wire",
}

BLOCKING_CALL = re.compile(r"\brunBlocking\s*(?:\(|\{)|\bThread\s*\.\s*sleep\s*\(|\.\s*join\s*\(")
GLOBAL_SCOPE = re.compile(r"\bGlobalScope\s*\.\s*(?:launch|async|coroutineContext)\b")
NOT_NULL_ASSERTION = re.compile(r"(?<![!=])!!(?![=])")
CATCH_HEAD = re.compile(r"\bcatch\s*\(")
BOOLEAN_PROPERTY = re.compile(r"\b(?:val|var)\s+(\w+)\s*:\s*Boolean\??\b")
TYPE_DECLARATION = re.compile(
    r"\b(?:data\s+|sealed\s+|abstract\s+|open\s+|inner\s+|value\s+|annotation\s+)*"
    r"(?:class|object|interface)\s+(\w+)"
)
FUNCTION_DECLARATION = re.compile(
    r"\bfun\s+(?:<[^>]*>\s*)?(?:([A-Za-z_][\w]*)\s*\.\s*)?(\w+)\s*\("
)
PROPERTY_DECLARATION = re.compile(r"^\s*(?:val|var)\s+(\w+)")


@dataclasses.dataclass(frozen=True)
class Block:
    """One brace block with the declaration header that introduced it."""

    open: int
    close: int
    header: str
    depth: int


@dataclasses.dataclass(frozen=True)
class Declaration:
    """The declaration that owns a block."""

    kind: str
    name: str
    receiver: str
    offset: int


@dataclasses.dataclass(frozen=True)
class Violation:
    """One red line hit in one file."""

    path: str
    line: int
    code: str
    detail: str
    subject: str

    def render(self) -> str:
        return f"{self.path}:{self.line}: [{self.code}] {self.detail}"


def _scan_string(source: str, index: int, triple: bool, keep: bool = False) -> tuple[str, int]:
    """Mask one string literal but keep its ``${...}`` template expressions."""
    length = len(source)
    delimiter = '"""' if triple else '"'
    cursor = index + len(delimiter)
    out: list[str] = [delimiter if keep else _blank(delimiter)]
    while cursor < length:
        if triple and source.startswith('"""', cursor):
            out.append('"""' if keep else _blank('"""'))
            return "".join(out), cursor + 3
        if not triple and source[cursor] == '"':
            out.append('"' if keep else _blank('"'))
            return "".join(out), cursor + 1
        if not triple and source[cursor] == "\n":
            return "".join(out), cursor
        if source.startswith("${", cursor):
            expression, cursor = _scan_template(source, cursor, keep=keep)
            out.append(expression)
            continue
        if source[cursor] == "\\" and cursor + 1 < length:
            out.append(source[cursor : cursor + 2] if keep else _blank(source[cursor : cursor + 2]))
            cursor += 2
            continue
        out.append(source[cursor] if keep else _blank(source[cursor]))
        cursor += 1
    return "".join(out), cursor


def _scan_template(source: str, index: int, keep: bool = False) -> tuple[str, int]:
    """Mask one ``${...}`` template expression, keeping its code."""
    length = len(source)
    cursor = index + 2
    depth = 1
    body: list[str] = []
    while cursor < length:
        character = source[cursor]
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0:
                body.append(character)
                if keep:
                    return source[index : cursor + 1], cursor + 1
                return "${" + mask_kotlin("".join(body[1:-1])) + "}", cursor + 1
        body.append(character)
        cursor += 1
    return (_blank(source[index:]) if not keep else source[index:]), cursor


def mask_comments(source: str) -> str:
    """Blank every comment but keep the string literals, preserving offsets."""
    out: list[str] = []
    index = 0
    length = len(source)
    while index < length:
        if source.startswith('"""', index):
            fragment, index = _scan_string(source, index, triple=True, keep=True)
            out.append(fragment)
            continue
        if source[index] == '"':
            fragment, index = _scan_string(source, index, triple=False, keep=True)
            out.append(fragment)
            continue
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = length if end < 0 else end
            out.append(_blank(source[index:end]))
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            out.append(_blank(source[index:end]))
            index = end
            continue
        out.append(source[index])
        index += 1
    return "".join(out)


def mask_kotlin(source: str) -> str:
    """Mask comments and string literals, preserving offsets and newlines."""
    out: list[str] = []
    index = 0
    length = len(source)
    while index < length:
        if source.startswith('"""', index):
            fragment, index = _scan_string(source, index, triple=True)
            out.append(fragment)
            continue
        if source[index] == '"':
            fragment, index = _scan_string(source, index, triple=False)
            out.append(fragment)
            continue
        if source[index] == "'":
            end = index + 1
            while end < length:
                if source[end] == "\\":
                    end += 2
                    continue
                if source[end] == "'" or source[end] == "\n":
                    end += 1
                    break
                end += 1
            out.append(_blank(source[index:end]))
            index = end
            continue
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = length if end < 0 else end
            out.append(_blank(source[index:end]))
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            out.append(_blank(source[index:end]))
            index = end
            continue
        out.append(source[index])
        index += 1
    return "".join(out)


def _blank(fragment: str) -> str:
    """Replace every non-newline character with a space."""
    return "".join("\n" if character == "\n" else " " for character in fragment)


def extract_comments(source: str) -> list[tuple[int, int, str]]:
    """Return the ``(start, end, text)`` of every comment in the source."""
    comments: list[tuple[int, int, str]] = []
    index = 0
    length = len(source)
    while index < length:
        if source.startswith('"""', index):
            _, index = _scan_string(source, index, triple=True)
            continue
        if source[index] == '"':
            _, index = _scan_string(source, index, triple=False)
            continue
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = length if end < 0 else end
            comments.append((index, end, source[index:end]))
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            comments.append((index, end, source[index:end]))
            index = end
            continue
        index += 1
    return comments


def _balanced(fragment: str) -> bool:
    """True when every parenthesis and bracket in the fragment is closed."""
    depth = 0
    for character in fragment:
        if character in "([":
            depth += 1
        elif character in ")]":
            depth -= 1
            if depth < 0:
                return False
    return depth == 0


def _header_text(masked: str, open_index: int) -> str:
    """The declaration header that introduced the block at ``open_index``."""
    depth = 0
    index = open_index - 1
    while index >= 0:
        character = masked[index]
        if character in ")]":
            depth += 1
        elif character in "([":
            if depth == 0:
                break
            depth -= 1
        elif depth == 0 and character in ";{}":
            break
        elif depth == 0 and character == "\n" and _balanced(masked[index + 1 : open_index]):
            break
        index -= 1
    return masked[index + 1 : open_index]


def scan_blocks(masked: str) -> list[Block]:
    """Every brace block of the masked source, with its declaration header."""
    blocks: list[Block] = []
    stack: list[tuple[int, str]] = []
    for index, character in enumerate(masked):
        if character == "{":
            stack.append((index, _header_text(masked, index)))
        elif character == "}" and stack:
            open_index, header = stack.pop()
            blocks.append(Block(open_index, index, header, len(stack)))
    return blocks


def _declaration_for(block: Block) -> Declaration | None:
    """The declaration that owns a block, when the header declares one."""
    header = block.header
    type_match = TYPE_DECLARATION.search(header)
    if type_match:
        return Declaration(
            kind="type",
            name=type_match.group(1),
            receiver="",
            offset=block.open - len(header) + type_match.start(1),
        )
    function_match = FUNCTION_DECLARATION.search(header)
    if function_match:
        return Declaration(
            kind="fun",
            name=function_match.group(2),
            receiver=function_match.group(1) or "",
            offset=block.open - len(header) + function_match.start(),
        )
    property_match = PROPERTY_DECLARATION.search(header)
    if property_match:
        return Declaration(
            kind="property",
            name=property_match.group(1),
            receiver="",
            offset=block.open - len(header) + property_match.start(1),
        )
    return None


def _line_of(masked: str, offset: int) -> int:
    return masked.count("\n", 0, offset) + 1


def _enclosing(blocks: list[Block], offset: int) -> list[Block]:
    """The blocks that contain the offset, outermost first."""
    return sorted(
        (block for block in blocks if block.open < offset < block.close),
        key=lambda block: block.open,
    )


def _is_phase_name(name: str) -> bool:
    """True when a Boolean property name describes one phase of a flow."""
    lowered = name.lower()
    for affix in PHASE_AFFIXES:
        if lowered.startswith(affix) and len(lowered) > len(affix):
            lowered = lowered[len(affix) :]
            break
    for affix in PHASE_AFFIXES:
        if lowered.endswith(affix) and len(lowered) > len(affix):
            lowered = lowered[: -len(affix)]
            break
    return lowered in PHASE_WORDS


def _is_deferred(header: str) -> bool:
    """True when the block body runs on another thread than the caller."""
    return any(re.search(rf"\b{builder}\b", header) for builder in DEFERRED_BUILDERS)


def _is_composable(masked: str, declaration: Declaration, blocks: list[Block]) -> bool:
    """True when the function carries the Compose ``@Composable`` annotation."""
    if declaration.kind != "fun":
        return False
    owner = next(
        (block for block in blocks if block.open < declaration.offset < block.close),
        None,
    )
    start = owner.open if owner is not None else 0
    prefix = masked[start : declaration.offset]
    return "@Composable" in prefix.split("fun")[-1] or prefix.rstrip().endswith("@Composable")


def _subject_for(blocks: list[Block], offset: int) -> str:
    """The symbol whose behaviour a narrow exception must regress."""
    declarations = [
        declaration
        for declaration in (_declaration_for(block) for block in _enclosing(blocks, offset))
        if declaration is not None
    ]
    if not declarations:
        return ""
    innermost = declarations[-1]
    if innermost.kind == "fun":
        owners = [item for item in declarations[:-1] if item.kind == "type"]
        if owners:
            return owners[-1].name
        if innermost.receiver:
            return innermost.receiver
    return innermost.name


def _waiver_reason(comments: list[tuple[int, int, str]], line: int, line_starts: list[int]) -> str:
    """The local explanation of a narrow exception, when the line carries one."""
    reasons: list[str] = []
    for start, end, text in comments:
        first_line = _line_index(line_starts, start)
        last_line = _line_index(line_starts, end - 1) if end > start else first_line
        if first_line > line or last_line < line - 1:
            continue
        marker = text.find(WAIVER_MARKER)
        if marker < 0:
            continue
        reason = text[marker + len(WAIVER_MARKER) :].strip().rstrip("*/").strip()
        reasons.append(reason)
    for reason in reasons:
        if len(reason) >= MINIMUM_REASON_LENGTH:
            return reason
    return ""


def _line_index(line_starts: list[int], offset: int) -> int:
    low = 0
    high = len(line_starts) - 1
    while low < high:
        middle = (low + high + 1) // 2
        if line_starts[middle] <= offset:
            low = middle
        else:
            high = middle - 1
    return low + 1


def find_violations(
    relative_path: str,
    source: str,
    test_symbols: set[str] | None = None,
) -> list[Violation]:
    """Every red line hit in one Kotlin source."""
    masked = mask_kotlin(source)
    blocks = scan_blocks(masked)
    comments = extract_comments(source)
    line_starts = [0]
    for index, character in enumerate(masked):
        if character == "\n":
            line_starts.append(index + 1)
    violations: list[Violation] = []

    def report(offset: int, code: str, detail: str, subject: str) -> None:
        line = _line_of(masked, offset)
        reason = _waiver_reason(comments, line, line_starts)
        if reason:
            if test_symbols is not None and subject and subject not in test_symbols:
                violations.append(
                    Violation(
                        relative_path,
                        line,
                        code,
                        f"narrow exception is not covered by a behaviour regression test for "
                        f"`{subject}`",
                        subject,
                    )
                )
            return
        violations.append(Violation(relative_path, line, code, detail, subject))

    for match in BLOCKING_CALL.finditer(masked):
        call = match.group(0)
        if call.startswith("runBlocking") or "sleep" in call:
            code = "ARCH001" if "sleep" in call else "ARCH002"
        else:
            code = "ARCH002"
        offset = match.start()
        enclosing = _enclosing(blocks, offset)
        deferred = any(_is_deferred(block.header) for block in enclosing)
        declarations = [
            declaration
            for declaration in (_declaration_for(block) for block in enclosing)
            if declaration is not None
        ]
        composable = any(
            declaration.kind == "fun" and _is_composable(masked, declaration, blocks)
            for declaration in declarations
        )
        main_thread_callback = any(
            declaration.kind == "fun" and declaration.name in MAIN_THREAD_CALLBACKS
            for declaration in declarations
        )
        presentation = any(marker in relative_path for marker in PRESENTATION_MARKERS)
        if code == "ARCH001":
            report(
                offset,
                code,
                f"{RULE_TITLES[code]} is forbidden in production code: `{call.strip()}`",
                _subject_for(blocks, offset),
            )
            continue
        if deferred:
            continue
        if composable or main_thread_callback or presentation:
            report(
                offset,
                code,
                f"{RULE_TITLES[code]} is forbidden on the Android main thread: `{call.strip()}`",
                _subject_for(blocks, offset),
            )

    for match in GLOBAL_SCOPE.finditer(masked):
        report(
            match.start(),
            "ARCH003",
            f"{RULE_TITLES['ARCH003']} is forbidden: `{match.group(0)}`",
            _subject_for(blocks, match.start()),
        )

    for match in NOT_NULL_ASSERTION.finditer(masked):
        report(
            match.start(),
            "ARCH006",
            f"{RULE_TITLES['ARCH006']} is forbidden in production code",
            _subject_for(blocks, match.start()),
        )

    for match in CATCH_HEAD.finditer(masked):
        open_paren = masked.find("(", match.start())
        close_paren = _matching(masked, open_paren)
        if close_paren < 0:
            continue
        caught = masked[open_paren + 1 : close_paren]
        block = next(
            (item for item in blocks if item.open > close_paren and item.open - close_paren < 8),
            None,
        )
        if block is None:
            continue
        body = masked[block.open + 1 : block.close]
        subject = _subject_for(blocks, match.start())
        if not body.strip():
            report(
                match.start(),
                "ARCH004",
                f"{RULE_TITLES['ARCH004']} is forbidden: `catch ({caught.strip()}) {{ }}`",
                subject,
            )
            continue
        if "CancellationException" in caught:
            name = caught.split(":")[0].strip()
            if not re.search(rf"\bthrow\s+{re.escape(name)}\b", body):
                report(
                    match.start(),
                    "ARCH005",
                    f"{RULE_TITLES['ARCH005']} is forbidden: "
                    f"`catch ({caught.strip()})` must rethrow the cancellation",
                    subject,
                )

    for match in TYPE_DECLARATION.finditer(masked):
        name = match.group(1)
        properties: list[str] = []
        cursor = match.end()
        while cursor < len(masked) and masked[cursor] in " \t\r\n":
            cursor += 1
        body: Block | None = None
        if cursor < len(masked) and masked[cursor] == "(":
            close = _matching(masked, cursor)
            if close < 0:
                continue
            properties.extend(
                item.group(1) for item in BOOLEAN_PROPERTY.finditer(masked[cursor + 1 : close])
            )
            cursor = close + 1
            while cursor < len(masked) and masked[cursor] in " \t\r\n":
                cursor += 1
        if cursor < len(masked) and masked[cursor] == "{":
            body = next(
                (item for item in blocks if item.open == cursor),
                None,
            )
        if body is not None:
            body_depth = len(_enclosing(blocks, body.open))
            for item in BOOLEAN_PROPERTY.finditer(masked, body.open, body.close):
                if len(_enclosing(blocks, item.start())) != body_depth + 1:
                    continue
                properties.append(item.group(1))
        phases = sorted({item for item in properties if _is_phase_name(item)})
        if len(phases) >= 2:
            report(
                match.start(1),
                "ARCH007",
                f"{RULE_TITLES['ARCH007']} is forbidden: "
                f"`{name}` carries the phase booleans {phases}",
                name,
            )
    return violations


def _matching(masked: str, open_index: int) -> int:
    """The index of the bracket that closes the one at ``open_index``."""
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[str] = []
    for index in range(open_index, len(masked)):
        character = masked[index]
        if character in pairs:
            stack.append(pairs[character])
        elif character in ")]}" and stack:
            if stack.pop() != character:
                return -1
            if not stack:
                return index
    return -1


def production_files(root: pathlib.Path) -> list[pathlib.Path]:
    """Every hand written Android production Kotlin file."""
    files: list[pathlib.Path] = []
    for relative in PRODUCTION_ROOTS:
        base = root / relative
        if not base.is_dir():
            continue
        for path in base.rglob("*.kt"):
            if any(marker in path.parts for marker in GENERATED_MARKERS):
                continue
            files.append(path)
    return sorted(files)


def test_symbols(root: pathlib.Path) -> set[str]:
    """Every symbol that a behaviour regression test mentions."""
    symbols: set[str] = set()
    for relative in TEST_ROOTS:
        base = root / relative
        if not base.is_dir():
            continue
        for path in base.rglob("*.kt"):
            source = path.read_text(encoding="utf-8", errors="replace")
            masked = mask_kotlin(source)
            readable = mask_comments(source)
            for block in scan_blocks(masked):
                declaration = _declaration_for(block)
                if declaration is None or declaration.kind != "fun":
                    continue
                header_start = max(0, block.open - len(block.header))
                prefix = readable[max(0, header_start - 200) : block.open]
                if "@Test" not in prefix:
                    continue
                body = readable[block.open : block.close]
                symbols.update(set(re.findall(r"\b[A-Za-z_]\w*\b", body)))
    return symbols


def scan_repository(root: pathlib.Path = ROOT) -> list[Violation]:
    """Every red line hit in the Android production tree."""
    symbols = test_symbols(root)
    violations: list[Violation] = []
    for path in production_files(root):
        relative = path.relative_to(root).as_posix()
        source = path.read_text(encoding="utf-8", errors="replace")
        violations.extend(find_violations(relative, source, symbols))
    return violations


#: The Lint issues that the project may silence, with the reason it owns.
APPROVED_LINT_SUPPRESSIONS = {
    "ForegroundServicePermission": (
        "MihomoVpnService is a VpnService, and the platform grants the "
        "`systemExempted` exemption to VPN apps configured in Settings > "
        "Network & Internet > VPN. The Lint detector only models the exact "
        "alarm exemption path."
    ),
    "AndroidGradlePluginVersion": (
        "The Gradle distribution is part of the reviewed AGP/Gradle/Kotlin/JDK "
        "tuple that scripts/android-toolchain.py checks."
    ),
    "NewerVersionAvailable": (
        "The detector queries Maven Central on every run, so the finding is "
        "not reproducible and the gate would depend on the network."
    ),
    "RedundantLabel": (
        "The Android manifest contract guard pins the launcher activity label "
        "to the registered product label."
    ),
}
#: The Detekt rules that this project does not enforce, and their owner.
APPROVED_DETEKT_EXCLUSIONS = {
    "complexity>LargeClass": "scripts/quality/kt-line-guard.py",
    "complexity>ComplexCondition": "scripts/quality/typed-state-guard.py",
    "complexity>CyclomaticComplexMethod": "scripts/quality/typed-state-guard.py",
    "complexity>LongMethod": "scripts/quality/kt-line-guard.py",
    "complexity>LongParameterList": "scripts/quality/typed-state-guard.py",
    "complexity>NestedBlockDepth": "scripts/quality/typed-state-guard.py",
    "complexity>TooManyFunctions": "scripts/quality/kt-line-guard.py",
    "exceptions>TooGenericExceptionCaught": "this guard",
    "performance>SpreadOperator": "the wire payload shape",
    "style>FunctionOnlyReturningConstant": "the request builders",
    "style>MagicNumber": "the typed wire owners",
    "style>MaxLineLength": "ktlint",
    "style>NewLineAtEndOfFile": "ktlint",
    "style>ReturnCount": "scripts/quality/typed-state-guard.py",
    "style>ThrowsCount": "this guard",
    "style>UseCheckOrError": "the host error contract",
    "style>UseRequire": "the host error contract",
    "style>WildcardImport": "ktlint",
}
#: The ktlint rules that a project may silence, and where it may do so.
APPROVED_KTLINT_DISABLES = {"ktlint_standard_property-naming": "*.kts"}
#: Files that would weaken a gate and must never exist.
FORBIDDEN_BASELINES = (
    "lint-baseline.xml",
    "detekt-baseline.xml",
    "ktlint-baseline.xml",
    ".editorconfig-baseline",
)


def configuration_problems(root: pathlib.Path = ROOT) -> list[str]:
    """The gate configuration must not be weakened behind the guard's back."""
    problems: list[str] = []
    android = root / "android"
    for path in sorted(android.rglob("*")):
        if path.is_file() and path.name in FORBIDDEN_BASELINES:
            problems.append(
                f"{path.relative_to(root).as_posix()}: a permanent lint baseline is forbidden"
            )
    gradle_path = root / "android/app/build.gradle.kts"
    if not gradle_path.is_file():
        problems.append("android/app/build.gradle.kts is missing")
        return problems
    gradle = gradle_path.read_text(encoding="utf-8")
    for required in (
        "abortOnError = true",
        "warningsAsErrors = true",
        "baseline = null",
        "ignoreFailures = false",
        'tasks.register("androidLintGate")',
        'dependsOn(tasks.named("lint$variant"))',
    ):
        if required not in gradle:
            problems.append(f"android/app/build.gradle.kts must contain `{required}`")
    for variant in ("ComposeDebug", "BevyDebug", "SlintDebug"):
        if f'"{variant}"' not in gradle:
            problems.append(
                f"android/app/build.gradle.kts must register the Lint task for the {variant} variant"
            )
    if re.search(r"maxIssues\s*=\s*[1-9]", gradle):
        problems.append("android/app/build.gradle.kts must not raise the Detekt maxIssues threshold")
    if re.search(r"\bbaseline\s*=\s*file\(", gradle) or re.search(r"\bbaseline\s*=\s*\"", gradle):
        problems.append("android/app/build.gradle.kts must not point ktlint or Detekt at a baseline file")

    lint_xml = root / "android/app/lint.xml"
    if not lint_xml.is_file():
        problems.append("android/app/lint.xml is missing")
    else:
        text = lint_xml.read_text(encoding="utf-8")
        found = set(re.findall(r'<issue\s+id="([^"]+)"', text))
        for issue in sorted(found - set(APPROVED_LINT_SUPPRESSIONS)):
            problems.append(f"android/app/lint.xml silences an unapproved Lint issue: {issue}")
        for issue in sorted(set(APPROVED_LINT_SUPPRESSIONS) - found):
            problems.append(f"android/app/lint.xml must document the {issue} suppression")
        if "ForegroundServicePermission" in found and not _declares_vpn_service(root):
            problems.append(
                "android/app/lint.xml may only silence ForegroundServicePermission while the "
                "manifest declares a VpnService"
            )

    detekt_yml = root / "android/config/detekt/detekt.yml"
    if not detekt_yml.is_file():
        problems.append("android/config/detekt/detekt.yml is missing")
    else:
        text = detekt_yml.read_text(encoding="utf-8")
        excluded: set[str] = set()
        ruleset = ""
        for line in text.splitlines():
            ruleset_match = re.match(r"^([a-zA-Z-]+):\s*$", line)
            if ruleset_match:
                ruleset = ruleset_match.group(1)
                continue
            rule_match = re.match(r"^\s{2}(\w+):\s*$", line)
            if not rule_match:
                continue
            window = text[text.index(line) : text.index(line) + 200]
            if re.search(r"active:\s*false", window):
                excluded.add(f"{ruleset}>{rule_match.group(1)}")
        for rule in sorted(excluded - set(APPROVED_DETEKT_EXCLUSIONS)):
            problems.append(f"android/config/detekt/detekt.yml excludes an unapproved rule: {rule}")
        for rule in sorted(set(APPROVED_DETEKT_EXCLUSIONS) - excluded):
            problems.append(f"android/config/detekt/detekt.yml must keep the documented scope: {rule}")
        if re.search(r"maxIssues\s*:", text):
            problems.append("android/config/detekt/detekt.yml must not raise the maxIssues threshold")

    editor_config = root / "android/.editorconfig"
    if not editor_config.is_file():
        problems.append("android/.editorconfig is missing")
    else:
        text = editor_config.read_text(encoding="utf-8")
        for key in sorted(set(re.findall(r"^(ktlint_\S+)\s*=\s*disabled", text, re.MULTILINE))):
            if key not in APPROVED_KTLINT_DISABLES:
                problems.append(f"android/.editorconfig disables an unapproved ktlint rule: {key}")
    return problems


def _declares_vpn_service(root: pathlib.Path) -> bool:
    """True while the production manifest still declares the VPN service."""
    manifest = root / "android/app/src/main/AndroidManifest.xml"
    if not manifest.is_file():
        return False
    text = manifest.read_text(encoding="utf-8")
    return "android.net.VpnService" in text and "BIND_VPN_SERVICE" in text


def self_test() -> int:
    """Positive and negative fixtures for every rule."""
    failures: list[str] = []

    def expect(label: str, source: str, code: str | None) -> None:
        found = find_violations("fixture/Fixture.kt", source)
        codes = {violation.code for violation in found}
        if code is None:
            if found:
                failures.append(f"{label}: expected no violation, found {sorted(codes)}")
            return
        if code not in codes:
            failures.append(f"{label}: expected {code}, found {sorted(codes) or 'nothing'}")
        for violation in found:
            if violation.code != code and code == "ARCH001" and violation.code == "ARCH002":
                continue

    expect("clean production code is accepted", "fun ok(): Int = 1", None)
    expect(
        "a production sleep is a red line",
        "class Owner {\n    fun wait() {\n        Thread.sleep(10)\n    }\n}",
        "ARCH001",
    )
    expect(
        "a main thread runBlocking is a red line",
        "class Owner {\n    fun onCreate() {\n        runBlocking { work() }\n    }\n}",
        "ARCH002",
    )
    expect(
        "a deferred runBlocking is accepted",
        "class Owner {\n    fun onCreate() {\n        scope.launch {\n            runBlocking { work() }\n        }\n    }\n}",
        None,
    )
    expect(
        "an unbounded global scope is a red line",
        "class Owner {\n    fun start() {\n        GlobalScope.launch { work() }\n    }\n}",
        "ARCH003",
    )
    expect(
        "an empty catch block is a red line",
        "class Owner {\n    fun read() {\n        try {\n            work()\n        } catch (error: Exception) {\n        }\n    }\n}",
        "ARCH004",
    )
    expect(
        "a swallowed cancellation is a red line",
        "class Owner {\n    fun read() {\n        try {\n            work()\n        } catch (error: CancellationException) {\n            report()\n        }\n    }\n}",
        "ARCH005",
    )
    expect(
        "a rethrown cancellation is accepted",
        "class Owner {\n    fun read() {\n        try {\n            work()\n        } catch (error: CancellationException) {\n            throw error\n        }\n    }\n}",
        None,
    )
    expect(
        "a not-null assertion is a red line",
        "class Owner {\n    fun read(value: String?) {\n        val text = value!!\n    }\n}",
        "ARCH006",
    )
    expect(
        "a multi-state Boolean wire is a red line",
        "data class Snapshot(\n    val isLoading: Boolean,\n    val isReady: Boolean,\n)",
        "ARCH007",
    )
    expect(
        "independent Boolean facts are accepted",
        "data class Snapshot(\n    val foreground: Boolean,\n    val ipv6: Boolean,\n)",
        None,
    )
    expect(
        "a string that looks like a red line is accepted",
        "class Owner {\n    val text = \"Thread.sleep(10) and !! and GlobalScope.launch\"\n}",
        None,
    )
    expect(
        "a comment that looks like a red line is accepted",
        "class Owner {\n    // Thread.sleep(10) is forbidden here\n    fun work() = Unit\n}",
        None,
    )
    expect(
        "a narrow exception without a reason is still a red line",
        "class Owner {\n    fun wait() {\n        // ARCH-REDLINE: too short\n        Thread.sleep(10)\n    }\n}",
        "ARCH001",
    )

    waiver = (
        "class Owner {\n"
        "    fun wait() {\n"
        "        // ARCH-REDLINE: the typed owner wait directive is bounded and runs off the main thread\n"
        "        Thread.sleep(10)\n"
        "    }\n"
        "}\n"
    )
    without_test = find_violations("fixture/Fixture.kt", waiver, set())
    if not any("behaviour regression" in violation.detail for violation in without_test):
        failures.append("narrow exception without a regression test: expected a violation")
    with_test = find_violations("fixture/Fixture.kt", waiver, {"Owner"})
    if with_test:
        failures.append(f"narrow exception with a regression test: expected none, found {with_test}")

    for failure in failures:
        print(f"  [FAIL] {failure}")
    if failures:
        print("self-test: FAIL")
        return 1
    print("self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--root", type=pathlib.Path, default=ROOT, help="repository root")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = args.root.resolve()
    violations = scan_repository(root)
    status = "enforce" if args.mode == "enforce" else "report"
    configuration = configuration_problems(root)
    if configuration:
        for problem in configuration:
            print(f"VIOLATION [{status}]: {problem}", file=sys.stderr)
    if violations or configuration:
        for violation in violations:
            print(f"VIOLATION [{status}]: {violation.render()}", file=sys.stderr)
        print(
            f"android architecture red-line guard: scanned={len(production_files(root))} "
            f"violations={len(violations)} configuration={len(configuration)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print(
        f"android architecture red-line guard: scanned={len(production_files(root))} "
        "violations=0 configuration=0"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
