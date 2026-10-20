#!/usr/bin/env python3
"""SemVer compatibility guard for the seven neutral crates (PUB-006.10).

The seven neutral ("Lurker") crates are published to crates.io. Once a version
is published, its public API is a promise: a consumer that compiles against
`lurker-core-contract 0.50.0` must keep compiling against the next release.
This guard makes that promise machine checked.

The guard has two layers, and both use the *same* baseline file.

1. Source layer (offline, always available).
   `scripts/quality/semver-baseline.json` records the public API surface of
   every neutral crate: every public item, its kind and its signature. The
   guard extracts the current surface from the crate sources with a
   deterministic lexical scanner and compares it with the baseline. A removed
   item or a changed signature is a breaking change and fails the gate; an
   added item is reported as drift.

2. Tool layer (`cargo-semver-checks`, optional).
   The baseline file also records the git revision the surface was reviewed
   at. When `cargo-semver-checks` is installed, the guard runs it against that
   revision for every neutral crate, so the type-checked analysis of the
   ecosystem tool backs the lexical one.

Modes
-----
structure (default, offline)
    The baseline exists, covers all seven neutral crates, carries the version
    this release publishes and a review revision.

check (offline)
    Compares the current public API with the baseline. Breaking changes fail;
    additive drift is reported.

verify (offline)
    The strict form: the baseline must be *exactly* the current public API.
    A release uses this form, so an unreviewed API can never be published.

capture (offline)
    Rewrites the baseline from the current sources. This is the deliberate
    act that accepts an API change.

negative (offline)
    The negative test: a copy of a neutral crate loses a public item, and the
    gate must reject that change. A gate that cannot fail is not a gate.

tool
    Runs `cargo-semver-checks` for every neutral crate when the tool is
    installed. A missing tool is reported, not silently ignored.

Usage
-----
    python3 scripts/quality/semver-compat-guard.py --mode structure
    python3 scripts/quality/semver-compat-guard.py --mode check
    python3 scripts/quality/semver-compat-guard.py --mode verify
    python3 scripts/quality/semver-compat-guard.py --mode capture
    python3 scripts/quality/semver-compat-guard.py --mode negative
    python3 scripts/quality/semver-compat-guard.py --mode tool
    python3 scripts/quality/semver-compat-guard.py --self-test
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

from rust_syntax import mask_comments, mask_noncode

ROOT = Path(__file__).resolve().parents[2]

# The neutral crates that are published to crates.io.
NEUTRAL_CRATES = (
    "lurker-core-contract",
    "lurker-core-ports",
    "lurker-engine-runtime",
    "lurker-profile-store",
    "lurker-ui-presentation",
    "mihomo-driver",
    "singbox-driver",
)

# The committed baseline of the public API surface.
BASELINE = Path("scripts/quality/semver-baseline.json")

# The schema version of the baseline file.
BASELINE_SCHEMA = 1

# A public item declaration. The visibility group is captured so that a
# restricted visibility (`pub(crate)`) can be excluded from the public surface.
ITEM_RE = re.compile(
    r"(?P<vis>pub(?:\s*\([^)]*\))?)\s+"
    r"(?P<kind>const\s+fn|async\s+fn|unsafe\s+fn|extern\s+fn|fn|struct|enum|trait|type|const|static|mod|union)\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)

# A public re-export.
REEXPORT_RE = re.compile(r"(?P<vis>pub(?:\s*\([^)]*\))?)\s+use\s+(?P<path>[^;]+);")

# A function, with or without an explicit `pub`. A trait method and a trait
# implementation method are public through the trait they belong to.
METHOD_RE = re.compile(
    r"(?P<vis>pub(?:\s*\([^)]*\))?)?\s*"
    r"(?:(?:const|async|unsafe|extern)\s+)*"
    r"(?P<kind>fn)\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)

# A public struct field.
FIELD_RE = re.compile(
    r"pub(?:\s*\([^)]*\))?\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*:\s*(?P<type>[^,;]+)"
)


@dataclass(frozen=True)
class ApiChange:
    """One difference between the baseline and the current public API."""

    crate: str
    key: str
    kind: str
    baseline: str
    current: str

    @property
    def breaking(self) -> bool:
        return self.kind in ("removed", "changed")


def _normalize(text: str) -> str:
    """Collapse whitespace so a formatting change is not an API change."""
    return re.sub(r"\s+", " ", text).strip()


def _strip_generics(text: str) -> str:
    """Remove every angle-bracket group from a type expression."""
    result = text
    while True:
        start = result.find("<")
        if start < 0:
            return result
        depth = 0
        for index in range(start, len(result)):
            if result[index] == "<":
                depth += 1
            elif result[index] == ">":
                depth -= 1
                if depth == 0:
                    result = result[:start] + " " + result[index + 1 :]
                    break
        else:
            return result[:start]


def _header_label(header: str) -> str:
    """The short label of a block header: the type or trait it declares."""
    cleaned = _normalize(header).rstrip("{").strip()
    cleaned = re.sub(r"^pub(\s*\([^)]*\))?\s+", "", cleaned)
    if cleaned.startswith("impl"):
        rest = cleaned[len("impl") :].strip()
        parts = rest.split(" for ")
        target = parts[-1].strip() if len(parts) > 1 else parts[0].strip()
        target = _strip_generics(target).strip()
        match = re.search(r"([A-Za-z_][A-Za-z0-9_:]*)", target)
        return match.group(1) if match else ""
    if cleaned.startswith("trait"):
        match = re.search(r"([A-Za-z_][A-Za-z0-9_]*)", _strip_generics(cleaned))
        return match.group(1) if match else ""
    if cleaned.startswith("mod"):
        match = re.search(r"([A-Za-z_][A-Za-z0-9_]*)", cleaned)
        return match.group(1) if match else ""
    return ""


def _blocks(text: str) -> list[tuple[int, int, str]]:
    """Every brace block of the masked source: (open, close, header)."""
    blocks: list[tuple[int, int, str]] = []
    stack: list[tuple[int, str]] = []
    statement_start = 0
    for index, char in enumerate(text):
        if char == "{":
            stack.append((index, text[statement_start:index]))
            statement_start = index + 1
        elif char == "}":
            if stack:
                open_index, header = stack.pop()
                blocks.append((open_index, index, header))
            statement_start = index + 1
        elif char == ";":
            statement_start = index + 1
    return blocks


def _declaration_head(text: str, start: int) -> tuple[str, int]:
    """The head of the declaration that starts at `start`, plus its body end."""
    index = start
    depth = 0
    while index < len(text):
        char = text[index]
        if char == "{" and depth == 0:
            return text[start:index], _block_end(text, index)
        if char in "{([":
            depth += 1
        elif char in "})]":
            depth -= 1
        elif char == ";" and depth == 0:
            return text[start:index], index
        index += 1
    return text[start:], len(text)


def _block_end(text: str, open_index: int) -> int:
    """The index after the brace that closes the block opened at `open_index`."""
    depth = 0
    for index in range(open_index, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index + 1
    return len(text)


def _body_members(body: str, kind: str) -> list[str]:
    """The public members a type contributes to its API."""
    members: list[str] = []
    cleaned = mask_comments(body)
    if kind == "struct":
        for match in FIELD_RE.finditer(cleaned):
            members.append(f"field {match.group('name')}: {_normalize(match.group('type'))}")
    elif kind == "enum":
        for name in _enum_variants(cleaned):
            members.append(f"variant {name}")
    elif kind == "trait":
        for match in METHOD_RE.finditer(cleaned):
            head, _ = _declaration_head(cleaned, match.start())
            members.append(_normalize(head))
    return members


def _enum_variants(body: str) -> list[str]:
    """The variant names of an enum body, in declaration order."""
    start = body.find("{")
    if start < 0:
        return []
    inner = body[start + 1 :]
    end = inner.rfind("}")
    if end >= 0:
        inner = inner[:end]
    variants: list[str] = []
    depth = 0
    current: list[str] = []
    for char in inner:
        if char in "{([":
            depth += 1
        elif char in "})]":
            depth -= 1
        if char == "," and depth == 0:
            variants.append("".join(current))
            current = []
            continue
        current.append(char)
    variants.append("".join(current))
    names: list[str] = []
    for variant in variants:
        entry = variant.strip()
        if not entry:
            continue
        match = re.match(r"([A-Z][A-Za-z0-9_]*)", entry)
        if match:
            names.append(match.group(1))
    return names


def _is_public(visibility: str) -> bool:
    """Only an unrestricted `pub` belongs to the public API surface."""
    return _normalize(visibility) == "pub"


def extract_crate_api(crate_dir: Path) -> dict[str, str]:
    """The public API surface of one crate: item key -> signature."""
    api: dict[str, str] = {}
    source_root = crate_dir / "src"
    if not source_root.is_dir():
        return api
    for source in sorted(source_root.rglob("*.rs")):
        masked = mask_noncode(source.read_text(encoding="utf-8", errors="replace"))
        stem = source.relative_to(crate_dir).with_suffix("").as_posix()
        blocks = _blocks(masked)
        for match in ITEM_RE.finditer(masked):
            context, trait_impl = _context_of(blocks, match.start())
            if not _is_public(match.group("vis")):
                # A method of a trait implementation is public through the
                # trait it implements, even without an explicit `pub`.
                if not (trait_impl and match.group("kind") == "fn"):
                    continue
            kind = match.group("kind").split()[-1]
            name = match.group("name")
            head, end = _declaration_head(masked, match.start())
            signature = _normalize(head)
            if kind in ("struct", "enum", "trait"):
                body = masked[match.end() : end]
                members = _body_members(body, kind)
                if members:
                    signature = f"{signature} {{ " + "; ".join(members) + " }"
            key = f"{stem}::{context}::{name}" if context else f"{stem}::{name}"
            api[key] = signature
        public_positions = {match.start() for match in ITEM_RE.finditer(masked)}
        for match in METHOD_RE.finditer(masked):
            if match.start() in public_positions:
                continue
            context, trait_impl = _context_of(blocks, match.start())
            if not trait_impl:
                continue
            head, _ = _declaration_head(masked, match.start())
            key = f"{stem}::{context}::{match.group('name')}"
            api[key] = _normalize(head)
        for match in REEXPORT_RE.finditer(masked):
            if not _is_public(match.group("vis")):
                continue
            path = _normalize(match.group("path"))
            context, _ = _context_of(blocks, match.start())
            key = f"{stem}::{context}::use {path}" if context else f"{stem}::use {path}"
            api[key] = f"pub use {path}"
    return api


def _context_of(blocks: list[tuple[int, int, str]], position: int) -> tuple[str, bool]:
    """The innermost labelled block that encloses `position`."""
    best: tuple[int, str] | None = None
    for open_index, close_index, header in blocks:
        if open_index < position < close_index:
            label = _header_label(header)
            if not label:
                continue
            if best is None or open_index > best[0]:
                best = (open_index, header)
    if best is None:
        return "", False
    header = _normalize(best[1]).rstrip("{").strip()
    is_trait_impl = header.startswith("impl") and " for " in header
    return _header_label(header), is_trait_impl


def collect_api(root: Path = ROOT) -> dict[str, dict[str, str]]:
    """The current public API surface of every neutral crate."""
    return {crate: extract_crate_api(root / "crates" / crate) for crate in NEUTRAL_CRATES}


def compare(baseline: dict[str, str], current: dict[str, str], crate: str) -> list[ApiChange]:
    """Every difference between the baseline surface and the current surface."""
    changes: list[ApiChange] = []
    for key, signature in sorted(baseline.items()):
        if key not in current:
            changes.append(ApiChange(crate, key, "removed", signature, ""))
        elif current[key] != signature:
            changes.append(ApiChange(crate, key, "changed", signature, current[key]))
    for key, signature in sorted(current.items()):
        if key not in baseline:
            changes.append(ApiChange(crate, key, "added", "", signature))
    return changes


def load_baseline(root: Path = ROOT) -> dict:
    path = root / BASELINE
    if not path.is_file():
        raise FileNotFoundError(f"{BASELINE}: the committed SemVer baseline is missing")
    return json.loads(path.read_text(encoding="utf-8"))


def write_baseline(root: Path, document: dict) -> None:
    path = root / BASELINE
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(document, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def _workspace_version(root: Path) -> str | None:
    import tomllib

    document = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    version = document.get("workspace", {}).get("package", {}).get("version")
    return version if isinstance(version, str) else None


def build_baseline(root: Path, revision: str) -> dict:
    """The baseline document of the current public API surface."""
    version = _workspace_version(root)
    crates: dict[str, dict] = {}
    for crate, api in collect_api(root).items():
        crates[crate] = {"version": version, "api": dict(sorted(api.items()))}
    return {
        "schema": BASELINE_SCHEMA,
        "generated_by": "scripts/quality/semver-compat-guard.py --mode capture",
        "workspace_version": version,
        "tool_baseline_revision": revision,
        "crates": crates,
    }


def structure_problems(root: Path = ROOT) -> list[str]:
    """The baseline must exist, be complete and match this release."""
    problems: list[str] = []
    try:
        document = load_baseline(root)
    except (FileNotFoundError, json.JSONDecodeError) as error:
        return [f"{BASELINE}: {error}"]
    if document.get("schema") != BASELINE_SCHEMA:
        problems.append(
            f"{BASELINE}: schema is {document.get('schema')!r}, expected {BASELINE_SCHEMA}"
        )
    version = _workspace_version(root)
    if document.get("workspace_version") != version:
        problems.append(
            f"{BASELINE}: the baseline was captured for version "
            f"{document.get('workspace_version')!r}, this release publishes {version!r}"
        )
    revision = document.get("tool_baseline_revision")
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        problems.append(
            f"{BASELINE}: `tool_baseline_revision` must be a 40 character git revision"
        )
    crates = document.get("crates")
    if not isinstance(crates, dict):
        problems.append(f"{BASELINE}: the `crates` table is missing")
        return problems
    for crate in NEUTRAL_CRATES:
        entry = crates.get(crate)
        if not isinstance(entry, dict):
            problems.append(f"{BASELINE}: the crate `{crate}` is not registered")
            continue
        api = entry.get("api")
        if not isinstance(api, dict) or not api:
            problems.append(f"{BASELINE}: the crate `{crate}` has no recorded public API")
        if entry.get("version") != version:
            problems.append(
                f"{BASELINE}: the crate `{crate}` was captured for version "
                f"{entry.get('version')!r}, this release publishes {version!r}"
            )
    for crate in sorted(set(crates) - set(NEUTRAL_CRATES)):
        problems.append(f"{BASELINE}: `{crate}` is not a neutral crate")
    return problems


def check_problems(root: Path = ROOT, strict: bool = False) -> tuple[list[str], list[ApiChange]]:
    """Compare the current surface with the baseline."""
    problems, changes = structure_problems(root), []
    if problems:
        return problems, changes
    document = load_baseline(root)
    for crate in NEUTRAL_CRATES:
        baseline = document["crates"][crate]["api"]
        current = extract_crate_api(root / "crates" / crate)
        for change in compare(baseline, current, crate):
            changes.append(change)
            if change.breaking or strict:
                detail = (
                    f" (baseline: {change.baseline!r}, current: {change.current!r})"
                    if change.kind == "changed"
                    else ""
                )
                problems.append(
                    f"{crate}: {change.kind} public item `{change.key}`{detail}"
                )
    return problems, changes


def _remove_first_public_function(text: str) -> tuple[str, str]:
    """Delete the first public function of a source file; return the new text."""
    masked = mask_noncode(text)
    for match in ITEM_RE.finditer(masked):
        if match.group("kind") != "fn" or not _is_public(match.group("vis")):
            continue
        _, end = _declaration_head(masked, match.start())
        return text[: match.start()] + text[end:], match.group("name")
    return text, ""


def negative_test(root: Path = ROOT) -> list[str]:
    """The negative test: a deliberate breaking change must be rejected."""
    problems: list[str] = []
    crate = "lurker-core-contract"
    source = root / "crates" / crate / "src" / "identity.rs"
    if not source.is_file():
        return [f"crates/{crate}/src/identity.rs: the negative test fixture is missing"]

    original = source.read_text(encoding="utf-8")
    mutated, removed = _remove_first_public_function(original)
    if not removed:
        return [f"crates/{crate}/src/identity.rs: the negative test found no public function"]

    with tempfile.TemporaryDirectory() as directory:
        sandbox = Path(directory) / crate
        (sandbox / "src").mkdir(parents=True)
        for path in sorted((root / "crates" / crate / "src").rglob("*.rs")):
            target = sandbox / path.relative_to(root / "crates" / crate)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(
                mutated if path.name == "identity.rs" else path.read_text(encoding="utf-8"),
                encoding="utf-8",
            )
        mutated_api = extract_crate_api(sandbox)

    try:
        document = load_baseline(root)
    except (FileNotFoundError, json.JSONDecodeError) as error:
        return [f"{BASELINE}: {error}"]
    baseline = document["crates"][crate]["api"]
    breaking = [change for change in compare(baseline, mutated_api, crate) if change.breaking]
    if not breaking:
        problems.append(
            "the negative test failed: removing a public function from "
            f"`crates/{crate}` was not reported as a breaking change"
        )
    else:
        print(
            "semver guard negative test: the gate rejected the deliberate break "
            f"({breaking[0].kind} `{breaking[0].key}`)"
        )

    clean = [
        change
        for change in compare(baseline, extract_crate_api(root / "crates" / crate), crate)
        if change.breaking
    ]
    if clean:
        problems.append(
            "the negative test failed: the unmodified crate already differs from the "
            "baseline in a breaking way"
        )
    return problems


def tool_problems(root: Path = ROOT, timeout: int = 3600) -> tuple[list[str], bool]:
    """Run `cargo-semver-checks` for every neutral crate."""
    problems: list[str] = []
    try:
        completed = subprocess.run(
            ["cargo", "semver-checks", "--version"],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return ([], False)
    if completed.returncode != 0:
        return ([], False)

    try:
        document = load_baseline(root)
    except (FileNotFoundError, json.JSONDecodeError) as error:
        return ([f"{BASELINE}: {error}"], True)
    revision = document.get("tool_baseline_revision")
    for crate in NEUTRAL_CRATES:
        command = [
            "cargo",
            "semver-checks",
            "check",
            "-p",
            crate,
            "--baseline-rev",
            revision,
            "--all-features",
        ]
        try:
            result = subprocess.run(
                command,
                cwd=root,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False,
            )
        except subprocess.TimeoutExpired:
            problems.append(f"{crate}: `cargo semver-checks` timed out")
            continue
        if result.returncode != 0:
            problems.append(
                f"{crate}: `cargo semver-checks` rejected the public API against {revision}"
            )
            for line in (result.stdout or "").strip().splitlines()[-20:]:
                problems.append(f"  {line}")
    return problems, True


def self_test() -> int:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        crate = root / "crates" / "demo"
        (crate / "src").mkdir(parents=True)
        (crate / "src/lib.rs").write_text(
            """
//! Demo crate.
pub struct Demo { pub value: u32 }
pub enum Mode { Idle, Busy }
pub fn run() -> u32 { 1 }
pub(crate) fn internal() -> u32 { 2 }
pub trait Contract { fn call(&self) -> u32; }
impl Contract for Demo { fn call(&self) -> u32 { self.value } }
""",
            encoding="utf-8",
        )
        api = extract_crate_api(crate)
        assert any(key.endswith("::Demo") for key in api), api
        assert any("field value" in value for value in api.values()), api
        assert any("variant Busy" in value for value in api.values()), api
        assert any(key.endswith("::run") for key in api), api
        assert not any("internal" in key for key in api), api
        assert any("fn call" in api[key] for key in api if key.endswith("::Contract")), api
        assert any("Demo::call" in key for key in api), api

        baseline = dict(api)
        current = dict(api)
        current.pop(next(key for key in current if key.endswith("::run")))
        changes = compare(baseline, current, "demo")
        assert [change.kind for change in changes] == ["removed"], changes
        assert changes[0].breaking

        current = dict(api)
        key = next(key for key in current if key.endswith("::run"))
        current[key] = "pub fn run() -> u64"
        changes = compare(baseline, current, "demo")
        assert [change.kind for change in changes] == ["changed"], changes
        assert changes[0].breaking

        current = dict(api)
        current["src/lib.rs::extra"] = "pub fn extra()"
        changes = compare(baseline, current, "demo")
        assert [change.kind for change in changes] == ["added"], changes
        assert not changes[0].breaking

    print("semver compatibility guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode",
        choices=("structure", "check", "verify", "capture", "negative", "tool"),
        default="structure",
        help="structure: baseline audit; check: breaking-change gate; verify: strict gate; "
        "capture: rewrite the baseline; negative: the negative test; tool: cargo-semver-checks",
    )
    parser.add_argument("--root", default=str(ROOT), help="repository root to audit")
    parser.add_argument(
        "--revision",
        default="",
        help="git revision the tool layer compares against (default: the current HEAD)",
    )
    parser.add_argument(
        "--require-tool",
        action="store_true",
        help="fail when `cargo-semver-checks` is not installed",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = Path(args.root).resolve()

    if args.mode == "structure":
        problems = structure_problems(root)
        if problems:
            for problem in problems:
                print(f"semver guard: {problem}", file=sys.stderr)
            print(f"semver guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
            return 1
        print(
            "semver guard: OK (committed baseline for "
            f"{len(NEUTRAL_CRATES)} neutral crates)"
        )
        return 0

    if args.mode == "capture":
        revision = args.revision
        if not revision:
            try:
                completed = subprocess.run(
                    ["git", "rev-parse", "HEAD"],
                    cwd=root,
                    capture_output=True,
                    text=True,
                    timeout=60,
                    check=False,
                )
                revision = completed.stdout.strip()
            except (FileNotFoundError, subprocess.TimeoutExpired):
                revision = ""
        if not re.fullmatch(r"[0-9a-f]{40}", revision):
            print(
                "semver guard: the tool baseline revision must be a 40 character git "
                "revision; pass it with --revision",
                file=sys.stderr,
            )
            return 1
        write_baseline(root, build_baseline(root, revision))
        print(f"semver guard: baseline captured ({BASELINE})")
        return 0

    if args.mode == "negative":
        problems = negative_test(root)
        if problems:
            for problem in problems:
                print(f"semver guard: {problem}", file=sys.stderr)
            print(f"semver guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
            return 1
        print("semver guard: OK (the deliberate breaking change was rejected)")
        return 0

    if args.mode == "tool":
        problems, available = tool_problems(root)
        if not available:
            message = "semver guard: `cargo-semver-checks` is not installed"
            if args.require_tool:
                print(message, file=sys.stderr)
                return 1
            print(message + "; the source layer stays authoritative")
            return 0
        if problems:
            for problem in problems:
                print(f"semver guard: {problem}", file=sys.stderr)
            print(f"semver guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
            return 1
        print("semver guard: OK (cargo-semver-checks accepted every neutral crate)")
        return 0

    problems, changes = check_problems(root, strict=args.mode == "verify")
    if problems:
        for problem in problems:
            print(f"semver guard: {problem}", file=sys.stderr)
        print(f"semver guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
        return 1
    drift = [change for change in changes if not change.breaking]
    for change in drift:
        print(
            f"semver guard: note: {change.crate}: added public item `{change.key}`; "
            "refresh the baseline with `--mode capture` before a release"
        )
    print(f"semver guard: OK ({len(changes)} difference(s), {len(drift)} additive)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
