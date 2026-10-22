#!/usr/bin/env python3
"""Prevent explicitly retired public product surfaces from returning.

The registry is deliberately exact and small. It records typed surface kinds,
their old spelling, replacement behavior tests, and an optional product-owned
source scope for identifiers that collide across independent surfaces.
Authoritative product documentation stays globally scanned; historical ledgers,
archives, build trees, and test fixtures are outside the source scan.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
import functools
import json
from pathlib import Path
import re
import subprocess
import sys
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
REGISTRY_PATH = Path("scripts/quality/retired-surfaces.json")
IGNORED_PARTS = {
    ".git",
    "archive",
    "archives",
    "build",
    "fixtures",
    "fixture",
    "fakes",
    "fake",
    "mocks",
    "mock",
    "target",
    "test",
    "tests",
    "vendor",
}


class Language(str, Enum):
    RUST = "rust"
    KOTLIN = "kotlin"
    CLI = "cli"


class SurfaceKind(str, Enum):
    PUBLIC_SYMBOL = "public_symbol"
    PRODUCT_COMMAND = "product_command"


@dataclass(frozen=True)
class RetiredSurface:
    id: str
    kind: SurfaceKind
    language: Language
    text: str
    rationale: str
    replacement_behavior_tests: tuple[str, ...]
    source_roots: tuple[str, ...]


def _is_test_file(path: Path) -> bool:
    name = path.name
    return bool(
        re.search(r"(?:^|[_-])tests?(?:[_-]|\.)", name, flags=re.IGNORECASE)
        or re.search(r"tests?\.(?:kt|rs)$", name, flags=re.IGNORECASE)
    )


def _is_ignored_path(path: Path) -> bool:
    return (
        path.name.lower() == "todo.md"
        or any(part.lower() in IGNORED_PARTS for part in path.parts)
        or _is_test_file(path)
    )


@functools.lru_cache(maxsize=None)
def _git_tracked_paths(root: str) -> frozenset[str] | None:
    """Return the git-tracked paths under `root`, or None when it is not a work tree.

    A retired-surface scope must point at source the repository actually owns.
    Generated and ignored build output never exists in a clean checkout, so
    guarding it would pass locally and fail in CI instead of catching anything.
    """
    if not (Path(root) / ".git").exists():
        return None
    try:
        result = subprocess.run(
            ["git", "-C", root, "ls-files", "-z"],
            capture_output=True,
            check=False,
        )
    except OSError:
        return None
    if result.returncode != 0:
        return None
    return frozenset(
        entry.decode("utf-8", errors="surrogateescape")
        for entry in result.stdout.split(b"\0")
        if entry
    )


def _is_tracked(root: Path, relative: Path) -> bool | None:
    """Return whether `relative` is tracked by git, or None outside a work tree."""
    tracked = _git_tracked_paths(str(root))
    if tracked is None:
        return None
    posix = relative.as_posix()
    return posix in tracked or any(path.startswith(f"{posix}/") for path in tracked)


def _parse_registry(root: Path) -> tuple[list[RetiredSurface], list[str]]:
    path = root / REGISTRY_PATH
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [], [f"{path}: cannot read retired-surface registry: {error}"]

    if not isinstance(document, dict) or document.get("version") != 1:
        return [], [f"{path}: expected registry object with version 1"]
    rows = document.get("retired_surfaces")
    if not isinstance(rows, list):
        return [], [f"{path}: `retired_surfaces` must be an array"]

    surfaces: list[RetiredSurface] = []
    problems: list[str] = []
    seen_ids: set[str] = set()
    seen_surfaces: set[tuple[SurfaceKind, Language, str]] = set()
    for index, row in enumerate(rows):
        prefix = f"{path}: retired_surfaces[{index}]"
        if not isinstance(row, dict):
            problems.append(f"{prefix} must be an object")
            continue
        try:
            surface = RetiredSurface(
                id=_required_string(row, "id"),
                kind=SurfaceKind(_required_string(row, "kind")),
                language=Language(_required_string(row, "language")),
                text=_required_string(row, "text"),
                rationale=_required_string(row, "rationale"),
                replacement_behavior_tests=_required_string_array(
                    row, "replacement_behavior_tests"
                ),
                source_roots=_optional_string_array(row, "source_roots"),
            )
        except (KeyError, TypeError, ValueError) as error:
            problems.append(f"{prefix}: {error}")
            continue

        if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", surface.id):
            problems.append(f"{prefix}: id must be a lowercase hyphenated slug")
        if surface.id in seen_ids:
            problems.append(f"{prefix}: duplicate id `{surface.id}`")
        seen_ids.add(surface.id)

        if not surface.text.strip():
            problems.append(f"{prefix}: text must not be empty")
        if surface.kind is SurfaceKind.PUBLIC_SYMBOL:
            if surface.language not in {Language.RUST, Language.KOTLIN}:
                problems.append(f"{prefix}: public symbols must use rust or kotlin language")
            if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", surface.text):
                problems.append(f"{prefix}: public symbol must be one exact identifier")
        elif surface.kind is SurfaceKind.PRODUCT_COMMAND:
            if surface.language is not Language.CLI:
                problems.append(f"{prefix}: product commands must use cli language")
            if len(surface.text.split()) < 2:
                problems.append(f"{prefix}: product command must be a product-specific phrase")

        key = (surface.kind, surface.language, surface.text)
        if key in seen_surfaces:
            problems.append(f"{prefix}: duplicate retired surface `{surface.text}`")
        seen_surfaces.add(key)

        if not surface.rationale.strip():
            problems.append(f"{prefix}: rationale must not be empty")
        if not surface.replacement_behavior_tests:
            problems.append(f"{prefix}: at least one replacement behavior test is required")
        for source_root in surface.source_roots:
            relative = Path(source_root)
            if relative.is_absolute() or ".." in relative.parts:
                problems.append(
                    f"{prefix}: source root must stay inside the repository: {source_root}"
                )
                continue
            if not (root / relative).is_dir() and not (root / relative).is_file():
                problems.append(f"{prefix}: source root is missing: {root / relative}")
                continue
            if _is_tracked(root, relative) is False:
                problems.append(
                    f"{prefix}: source root is not tracked by git: {source_root}"
                    " (generated or ignored output cannot be guarded;"
                    " scope the surface to tracked source)"
                )
        surfaces.append(surface)

    if not surfaces and not problems:
        problems.append(f"{path}: registry must contain at least one evidenced retired surface")
    return surfaces, problems


def _required_string(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str):
        raise TypeError(f"`{key}` must be a string")
    return value


def _required_string_array(row: dict[str, Any], key: str) -> tuple[str, ...]:
    value = row.get(key)
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise TypeError(f"`{key}` must be an array of strings")
    return tuple(value)


def _optional_string_array(row: dict[str, Any], key: str) -> tuple[str, ...]:
    value = row.get(key, [])
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise TypeError(f"`{key}` must be an array of strings")
    return tuple(value)


def _replacement_test_problems(root: Path, surface: RetiredSurface) -> list[str]:
    problems: list[str] = []
    for raw_path in surface.replacement_behavior_tests:
        relative = Path(raw_path)
        if relative.is_absolute() or ".." in relative.parts:
            problems.append(
                f"{surface.id}: replacement behavior test path must stay inside the repository: {raw_path}"
            )
            continue
        path = root / relative
        if not path.is_file():
            problems.append(f"{surface.id}: replacement behavior test is missing: {path}")
            continue
        normalized = relative.as_posix()
        in_test_location = any(
            part.lower() in {"test", "tests"}
            or part.lower().endswith(("_test", "_tests"))
            for part in relative.parts
        )
        named_as_test = bool(
            re.search(r"(?:^|[_-])tests?(?:[_-]|\.)", relative.name, re.IGNORECASE)
            or re.search(r"tests?\.(?:kt|rs)$", relative.name, re.IGNORECASE)
        )
        if not in_test_location and not named_as_test:
            problems.append(
                f"{surface.id}: replacement behavior test path is not clearly a test: {normalized}"
            )
    return problems


def _source_paths(root: Path, language: Language) -> list[Path]:
    paths: set[Path] = set()
    if language is Language.RUST:
        for source_root in (root / "crates", root / "android"):
            if source_root.is_dir():
                paths.update(source_root.rglob("*.rs"))
    if language is Language.KOTLIN:
        android_root = root / "android"
        if android_root.is_dir():
            paths.update(android_root.rglob("src/main/**/*.kt"))
    if language is Language.CLI:
        cli_root = root / "crates/infiltrator-cli"
        cli_source = cli_root / "src"
        if cli_source.is_dir():
            paths.update(cli_source.rglob("*.rs"))
        manifest = cli_root / "Cargo.toml"
        if manifest.is_file():
            paths.add(manifest)
        scripts_root = root / "scripts"
        if scripts_root.is_dir():
            paths.update(scripts_root.rglob("*.sh"))

    return sorted(
        path
        for path in paths
        if not _is_ignored_path(path.relative_to(root))
        and not (language is Language.CLI and path.is_relative_to(root / "scripts/quality"))
    )


def _documentation_paths(root: Path) -> list[Path]:
    paths: set[Path] = set()
    for root_doc in (root / "README.md", root / "TESTING.md"):
        if root_doc.is_file():
            paths.add(root_doc)

    doc_roots = [root / "docs", root / "android", root / "crates"]
    for doc_root in doc_roots:
        if not doc_root.is_dir():
            continue
        for path in doc_root.rglob("*.md"):
            if _is_ignored_path(path.relative_to(root)):
                continue
            if path.name == "README.md" or "docs" in path.relative_to(root).parts:
                paths.add(path)
    return sorted(paths)


def _surface_pattern(surface: RetiredSurface) -> re.Pattern[str]:
    if surface.kind is SurfaceKind.PUBLIC_SYMBOL:
        return re.compile(rf"\b{re.escape(surface.text)}\b")
    return re.compile(
        rf"(?<![A-Za-z0-9_-]){re.escape(surface.text)}(?![A-Za-z0-9_-])"
    )


def _matching_lines(path: Path, pattern: re.Pattern[str]) -> tuple[list[int], str | None]:
    try:
        content = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        return [], f"cannot scan file: {error}"
    return (
        [
            content.count("\n", 0, match.start()) + 1
            for match in pattern.finditer(content)
        ],
        None,
    )


def _all_source_paths(root: Path) -> list[Path]:
    """Return every source file the retired-surface scan may read."""
    paths: set[Path] = set()
    for language in Language:
        paths.update(_source_paths(root, language))
    return sorted(paths)


IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def _relocated_identifiers(
    root: Path, surfaces: list[RetiredSurface]
) -> dict[str, set[str]]:
    """Map each retired identifier to the scopes it still appears in.

    A retired surface is often *relocated* instead of deleted: the owner moves
    to another crate and keeps the name. Documentation that names such a
    surface describes the current product, so it is not a resurrection. The
    map records, for every retired identifier, the set of scope roots it
    still appears under plus the empty string when it appears outside every
    scope. A surface whose identifier appears outside its own scope is
    therefore alive under a new owner.
    """
    wanted = {
        surface.text for surface in surfaces if surface.kind is SurfaceKind.PUBLIC_SYMBOL
    }
    scopes = sorted(
        {
            source_root
            for surface in surfaces
            for source_root in surface.source_roots
        }
    )
    appearances: dict[str, set[str]] = {text: set() for text in wanted}
    if not wanted:
        return appearances
    for path in _all_source_paths(root):
        try:
            content = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        hits = set(IDENTIFIER.findall(content)) & wanted
        if not hits:
            continue
        relative = path.relative_to(root).as_posix()
        matched = {
            scope
            for scope in scopes
            if relative == scope or relative.startswith(f"{scope}/")
        }
        for token in hits:
            appearances[token].update(matched or {""})
    return appearances


def check_surfaces(root: Path, surfaces: list[RetiredSurface]) -> list[str]:
    problems: list[str] = []
    docs = _documentation_paths(root)
    appearances: dict[str, set[str]] | None = None
    for surface in surfaces:
        problems.extend(_replacement_test_problems(root, surface))
        pattern = _surface_pattern(surface)
        sources = _source_paths(root, surface.language)
        if surface.source_roots:
            roots = tuple(root / source_root for source_root in surface.source_roots)
            sources = [
                path
                for path in sources
                if any(path == source_root or path.is_relative_to(source_root) for source_root in roots)
            ]
        for path in sources:
            lines, read_error = _matching_lines(path, pattern)
            if read_error is not None:
                problems.append(f"{path}: {read_error}")
            for line in lines:
                problems.append(
                    f"{path}:{line}: retired surface `{surface.text}` ({surface.id}) has reappeared"
                )
        if surface.kind is not SurfaceKind.PUBLIC_SYMBOL:
            for path in docs:
                lines, read_error = _matching_lines(path, pattern)
                if read_error is not None:
                    problems.append(f"{path}: {read_error}")
                for line in lines:
                    problems.append(
                        f"{path}:{line}: retired surface `{surface.text}` ({surface.id}) has reappeared"
                    )
            continue
        if appearances is None:
            appearances = _relocated_identifiers(root, surfaces)
        scope = set(surface.source_roots)
        if appearances.get(surface.text, set()) - scope:
            # The identifier is still live under another owner: the product
            # documentation names the current symbol, not a resurrected one.
            continue
        for path in docs:
            lines, read_error = _matching_lines(path, pattern)
            if read_error is not None:
                problems.append(f"{path}: {read_error}")
            for line in lines:
                problems.append(
                    f"{path}:{line}: retired surface `{surface.text}` ({surface.id}) has reappeared"
                )
    return problems


def check(root: Path = REPO_ROOT) -> list[str]:
    surfaces, registry_problems = _parse_registry(root)
    return registry_problems + check_surfaces(root, surfaces)


def main() -> int:
    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1
    print("retired surface guard: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
