#!/usr/bin/env python3
"""Require one Application-owned Android owner IPC client instance."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[2]
SOURCE_ROOT = Path("android/app/src/main/java/com/musicfrog/infiltrator")
OWNER_SOURCE = "android/app/src/main/java/com/musicfrog/infiltrator/MihomoApplication.kt"
SHARED_CLIENT_CONSUMERS = (
    "android/app/src/main/java/com/musicfrog/infiltrator/CoreHost.kt",
    "android/app/src/main/java/com/musicfrog/infiltrator/BevyHostActivity.kt",
    "android/app/src/main/java/com/musicfrog/infiltrator/InfiltratorTileService.kt",
)
CLIENT_CONSTRUCTOR = re.compile(r"\bMihomoControlClient\s*\(")
CLIENT_CLASS = re.compile(r"\bclass\s+MihomoControlClient\s*\(")


def code_without_comments(source: str) -> str:
    source = re.sub(r"/\*.*?\*/", "", source, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", source)


def source_problems_from_sources(sources: dict[str, str]) -> list[str]:
    problems: list[str] = []
    owner_source = code_without_comments(sources.get(OWNER_SOURCE, ""))
    if "internal val ownerIpcClient: MihomoControlClient by ownerIpcClientDelegate" not in owner_source:
        problems.append(f"{OWNER_SOURCE}: MihomoControlClient must be owned by the Application")
    if "MihomoControlClient(applicationContext)" not in owner_source:
        problems.append(f"{OWNER_SOURCE}: the Application must be the only constructor call")

    constructors: list[str] = []
    for path, source in sources.items():
        code = code_without_comments(source)
        code = CLIENT_CLASS.sub("class MihomoControlClient ", code)
        constructors.extend(path for _ in CLIENT_CONSTRUCTOR.finditer(code))
    if constructors != [OWNER_SOURCE]:
        problems.append(
            "Android production sources must contain exactly one MihomoControlClient constructor call in MihomoApplication.kt"
        )

    for consumer in SHARED_CLIENT_CONSUMERS:
        code = code_without_comments(sources.get(consumer, ""))
        if "ownerIpcClient" not in code:
            problems.append(f"{consumer}: must reuse the Application-owned ownerIpcClient")
    return problems


def source_problems(repo_root: Path = ROOT) -> list[str]:
    sources: dict[str, str] = {}
    source_root = repo_root / SOURCE_ROOT
    if not source_root.is_dir():
        return [f"{source_root}: Android production source directory is missing"]
    for path in source_root.rglob("*.kt"):
        relative = path.relative_to(repo_root).as_posix()
        try:
            sources[relative] = path.read_text(encoding="utf-8")
        except OSError as error:
            return [f"{path}: could not read Kotlin source: {error}"]
    return source_problems_from_sources(sources)


def self_test() -> bool:
    valid = {
        OWNER_SOURCE: """
            private val ownerIpcClientDelegate = lazy { MihomoControlClient(applicationContext) }
            internal val ownerIpcClient: MihomoControlClient by ownerIpcClientDelegate
        """,
        SHARED_CLIENT_CONSUMERS[0]: "val client = application.ownerIpcClient",
        SHARED_CLIENT_CONSUMERS[1]: "fun ownerIpcClient() = application.ownerIpcClient",
        SHARED_CLIENT_CONSUMERS[2]: "private val client by lazy { app.ownerIpcClient }",
        "android/app/src/main/java/com/musicfrog/infiltrator/MihomoControlClient.kt":
            "internal class MihomoControlClient(context: Context) {}",
    }
    if source_problems_from_sources(valid):
        return False

    duplicate = dict(valid)
    duplicate[SHARED_CLIENT_CONSUMERS[2]] += "\nval second = MihomoControlClient(context)"
    if not any("exactly one MihomoControlClient constructor" in issue
               for issue in source_problems_from_sources(duplicate)):
        return False

    missing_owner = dict(valid)
    missing_owner[OWNER_SOURCE] = "internal val ownerIpcClient: MihomoControlClient by lazy { MihomoControlClient(context) }"
    if not any("only constructor call" in issue for issue in source_problems_from_sources(missing_owner)):
        return False

    missing_consumer = dict(valid)
    missing_consumer[SHARED_CLIENT_CONSUMERS[1]] = "MihomoControlClient(context).readSnapshot()"
    if not any(SHARED_CLIENT_CONSUMERS[1] in issue
               for issue in source_problems_from_sources(missing_consumer)):
        return False
    return True


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    if args.self_test:
        if self_test():
            print("Android owner client guard self-test: passed")
            return 0
        print("Android owner client guard self-test: failed", file=sys.stderr)
        return 1

    problems = source_problems()
    if problems:
        for problem in problems:
            print(f"VIOLATION: {problem}", file=sys.stderr)
        return 1
    print("Android owner client guard: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
