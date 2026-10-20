#!/usr/bin/env python3
"""Render the platform evidence matrix into `docs/PLATFORM_MATRIX.md`.

The evidence ledger (`scripts/platform/evidence/*.json`) is the single source
of truth; this script projects it into the generated section of the platform
matrix document. The document never carries a hand-written second copy of the
matrix, and the guard re-renders the section to prove that it did not drift.

Usage:
    python3 scripts/platform/build-evidence-matrix.py            # rewrite the section
    python3 scripts/platform/build-evidence-matrix.py --check    # fail on drift
    python3 scripts/platform/build-evidence-matrix.py --print    # print the section

Exit codes: 0 the section is in sync, 1 the ledger or the document is invalid,
2 the document has no generated section to write into.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from evidence_model import (  # noqa: E402  (the model lives next to this script)
    PLATFORM_MATRIX,
    ROOT,
    ledger_problems,
    load_ledgers,
    render_section,
    replace_section,
    section_span,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="only report the drift")
    parser.add_argument("--print", dest="show", action="store_true", help="print the section")
    arguments = parser.parse_args()

    ledgers = load_ledgers()
    problems: list[str] = []
    for ledger in ledgers:
        problems.extend(ledger_problems(ledger))
    if problems:
        for problem in problems:
            print(f"platform evidence: {problem}", file=sys.stderr)
        return 1

    section = render_section(ledgers)
    if arguments.show:
        print(section)
        return 0

    document_path = ROOT / PLATFORM_MATRIX
    document = document_path.read_text(encoding="utf-8")
    try:
        start, end = section_span(document)
    except ValueError as error:
        print(f"platform evidence: {error}", file=sys.stderr)
        return 2

    if arguments.check:
        if document[start:end] != section:
            print(
                "platform evidence: the generated section drifted from the ledger; "
                "run python3 scripts/platform/build-evidence-matrix.py",
                file=sys.stderr,
            )
            return 1
        print("platform evidence: the generated section is in sync with the ledger")
        return 0

    document_path.write_text(replace_section(document, section), encoding="utf-8")
    print(f"platform evidence: the generated section of {PLATFORM_MATRIX} was rewritten")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
