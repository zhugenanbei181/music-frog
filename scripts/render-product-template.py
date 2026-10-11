#!/usr/bin/env python3
"""Render a packaging template from the product identity registry."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys

from product_identity import read_properties, render_template


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    try:
        content = arguments.input.read_text(encoding="utf-8")
        rendered = render_template(content, read_properties())
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(rendered, encoding="utf-8")
    except (OSError, ValueError) as error:
        print(f"product template: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
