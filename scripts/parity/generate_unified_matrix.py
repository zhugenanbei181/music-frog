#!/usr/bin/env python3
"""Generates the unified 182-scenario matrix for Iced, Bevy, and Slint surfaces.

Matrix specification:
- 118 business scenarios: 59 features x 2 viewports (standard + compact), default forest skin
- 64 theme inspection scenarios: 16 core pages x 4 themes (dark, light, forest, amoled)
Total: exactly 182 scenarios per surface.

Columns are `name page skin window_size` for every surface, plus three columns
for the Iced surface only: the contract `shape` the frame is judged by, the
`interaction` fixture it opens (empty for a plain page), and the
`sidebar_expansion` preference the fixture publishes (only the surface-owned
collapsed-sidebar rows fill that one).

Slint's committed matrix extends that shared header with five capture-owned
columns and a `locale` column. The generator checks/rewrites its own four
leading columns and preserves the rest verbatim, so a locale row is a surface
fact rather than a second contract matrix.

`page` is the selector the surface's own capture opens (a route, a pane or a
section anchor). For Slint those selectors already ARE the contract's page
shapes - `connections-details-drawer-standard` opens `connections_workbench`
while `connections-dark` opens the framed `connections` page - so its frames
carry the shape they must be judged by. Iced's selector is a route
(`runtime`, `mixin`, `filter`), which is not a shape at all, so the shape it
paints has to travel beside it. The generator derives it from the contract's
own registry: the same feature's Slint page is the shape of the same business
view, and a feature Slint does not implement falls back to the shape of its
unified page. `scripts/parity/test_generate_unified_matrix.py` holds every
emitted value to the exported `page-structure.json`.

The committed matrices also carry capture rows the generator does not own (the
mascot frames, the mobile viewports, the collapsed-sidebar frames). They are
kept exactly where the committed file has them - some sit in the middle of the
generated block, not only at its end - and `--check` proves every generated row
is present, in generated order and byte for byte.
"""

import re
import sys
from pathlib import Path
import json

ROOT = Path(__file__).resolve().parents[2]
PARITY_RS = ROOT / "crates/infiltrator-contract/src/parity.rs"
PAGE_STRUCTURE = ROOT / "crates/infiltrator-contract/assets/page-structure.json"

def parse_features():
    content = PARITY_RS.read_text(encoding="utf-8")
    pattern = re.compile(
        r'([A-Za-z0-9_]+)\s*=>\s*\(\s*"([^"]+)"\s*,\s*"([^"]*)"\s*,\s*"([^"]*)"\s*,\s*"([^"]*)"'
    )
    matches = pattern.findall(content)
    features = []
    for var, fid, iced, bevy, slint in matches:
        if fid == "shell-keyboard-focus":
            continue
        features.append((fid, iced, bevy, slint))
    return features

CORE_PAGES = [
    ("overview", "overview", "overview", "overview"),
    ("proxies", "proxies", "proxies", "proxies"),
    ("rules", "rules", "rules", "rules"),
    ("settings", "settings", "settings", "settings"),
    ("profiles", "profiles", "profiles", "profiles"),
    ("connections", "connections", "connections", "connections"),
    ("logs", "logs", "logs", "logs"),
    ("runtime", "runtime", "overview", "runtime"),
    ("dns", "dns", "dns", "dns"),
    ("doctor", "doctor", "doctor", "doctor"),
    ("sync", "sync", "sync", "sync"),
    ("app-routing", "app_routing", "app_routing", "app_routing"),
    ("editor", "editor", "profiles", "editor"),
    ("mixin", "mixin", "profiles", "editor"),
    ("filter", "filter", "profiles", "filters"),
    ("about", "settings", "settings", "about"),
]

# The unified page identity of a capture is not always spelled the way the
# contract's exported shape table spells it: the export names the shapes
# `app_routing`, `filters` and `editor`, while the unified matrix calls those
# pages `app-routing`, `filter` and `mixin`. Two names for one fact, resolved
# here once instead of in every consumer.
UNIFIED_PAGE_SHAPES = {
    "app-routing": "app_routing",
    "filter": "filters",
    "mixin": "editor",
}

# The shape of an Iced page selector whose feature has no Slint counterpart to
# borrow from. Every value is one the exported table owns.
ICED_PAGE_SHAPES = {
    "mixin": "editor",
    "filter": "filters",
    "rules-providers": "rules",
    "sync-restore-confirm": "sync",
}


def exported_shapes() -> set[str]:
    """The page shapes the contract exports; the derived column may use no other."""
    document = json.loads(PAGE_STRUCTURE.read_text(encoding="utf-8"))
    return {entry["slug"] for entry in document.get("slugs", [])}


def iced_shape(iced: str, bevy: str, slint: str) -> str:
    """The contract page shape an Iced capture of this feature paints.

    The same business view has one body shape: the Slint column of the very
    same registry row already names it (that column IS the shape the Slint
    frame is judged by), so an Iced capture of that feature is judged by the
    same fact. A feature Slint does not carry falls back to the shape of the
    Iced page it opens, and a feature neither surface implements to the
    overview page's shape - never to a shape the export does not own.
    """
    shapes = exported_shapes()
    if slint in shapes:
        return slint
    unified = iced or bevy or "overview"
    shape = ICED_PAGE_SHAPES.get(unified, unified)
    if shape not in shapes:
        raise ValueError(
            f"no exported page shape for feature page {unified!r} (iced={iced!r} "
            f"bevy={bevy!r} slint={slint!r})"
        )
    return shape

# Business captures use the eye-care forest skin by default. Theme inspection
# remains the explicit four-theme matrix so every skin still has real pixels.
BUSINESS_SKIN = "forest"
THEMES = ["dark", "light", "forest", "amoled"]

def generate_surface_tsv(surface: str, out_path: Path):
    features = parse_features()
    if surface == "iced":
        lines = [
            "name\tpage\tshape\tskin\twindow_size\tinteraction\tsidebar_expansion"
            "\ttraffic_audit\n"
        ]
    else:
        lines = ["name\tpage\tskin\twindow_size\n"]

    def row(
        name: str, page: str, shape: str, skin: str, window: str, interaction: str = ""
    ) -> str:
        if surface == "iced":
            # The eighth column is the traffic audit's own selection; a row that
            # does not name one keeps the reader's default.
            return f"{name}\t{page}\t{shape}\t{skin}\t{window}\t{interaction}\t\t\n"
        return f"{name}\t{page}\t{skin}\t{window}\n"

    # 1. 118 Business scenarios (alphabetical by feature id)
    for fid, iced, bevy, slint in sorted(features, key=lambda x: x[0]):
        shape = ""
        if surface == "iced":
            page = "editor" if fid == "sync-restore-confirm" else (iced if iced else "overview")
            std_dim = "1180x780"
            shape = iced_shape(iced, bevy, slint)
        elif surface == "bevy":
            page = bevy if bevy else "overview"
            std_dim = "1180x780"
        elif surface == "slint":
            page = slint if slint else "Home"
            std_dim = "1180x780"
        else:
            raise ValueError(f"Unknown surface: {surface}")

        lines.append(row(f"{fid}-standard", page, shape, BUSINESS_SKIN, std_dim, fid))
        lines.append(row(f"{fid}-compact", page, shape, BUSINESS_SKIN, "720x480", fid))

    # 2. 64 Theme inspection scenarios (16 pages x 4 themes)
    for page_name, iced_p, bevy_p, slint_p in CORE_PAGES:
        shape = UNIFIED_PAGE_SHAPES.get(page_name, page_name)
        for theme in THEMES:
            if surface == "iced":
                p = iced_p
                std_dim = "1180x780"
            elif surface == "bevy":
                p = bevy_p
                std_dim = "1180x780"
            elif surface == "slint":
                p = slint_p
                std_dim = "1180x780"
            lines.append(row(f"{page_name}-{theme}", p, shape, theme, std_dim))

    out_path.write_text("".join(lines), encoding="utf-8")
    scenario_count = len(lines) - 1
    print(f"[{surface}] Wrote {scenario_count} scenarios to {out_path}")
    assert scenario_count == 182, f"Expected 182 scenarios, got {scenario_count}"

def generated_rows(surface: str) -> list[str]:
    """The generator's own rows, header first, without touching the worktree."""
    import tempfile

    with tempfile.TemporaryDirectory() as directory:
        fresh_path = Path(directory) / f"{surface}.tsv"
        generate_surface_tsv(surface, fresh_path)
        return fresh_path.read_text(encoding="utf-8").splitlines()


def merge_line(generated: str, committed: str) -> str:
    """One row: the generator owns the columns it emits, the surface the rest.

    The Slint matrix carries six columns the generator does not produce
    (`scroll`, `mascot_frame`, `mascot_millis`, `sidebar_expansion`,
    `interaction`, `locale`); those belong to the surface's capture path. When
    the generator's own leading columns match, the committed line is kept
    verbatim; when they do not, the generator's columns win and the surface's
    trailing columns ride along, so a page change never silently drops an
    interaction or a locale.
    """
    generated_fields = generated.split("\t")
    committed_fields = committed.split("\t")
    if committed_fields[: len(generated_fields)] == generated_fields:
        return committed
    return "\t".join(generated_fields + committed_fields[len(generated_fields) :])


def merge_rows(fresh: list[str], committed: list[str]) -> tuple[list[str], int]:
    """Generated rows in generated order, surface-owned rows where they were.

    A surface-owned row is any committed row the generator does not emit. It is
    anchored to the generated row it follows in the committed file (or to the
    header when it comes first), so regenerating reproduces the committed file
    byte for byte and can never delete or reorder one. Anchoring is not
    cosmetic: twelve of the Slint rows sit in the middle of the generated block,
    and a rule that only kept a trailing tail silently deleted them.
    """
    generated_names = {line.split("\t")[0] for line in fresh[1:]}
    committed_by_name: dict[str, str] = {}
    anchored: dict[str, list[str]] = {}
    anchor = fresh[0].split("\t")[0]
    for line in committed[1:]:
        if not line.strip():
            continue
        name = line.split("\t")[0]
        if name in generated_names:
            committed_by_name.setdefault(name, line)
            anchor = name
            continue
        anchored.setdefault(anchor, []).append(line)
    merged = [
        merge_line(fresh[0], committed[0])
        if committed and committed[0].split("\t")[0] == fresh[0].split("\t")[0]
        else fresh[0]
    ]
    for line in fresh[1:]:
        name = line.split("\t")[0]
        merged.append(merge_line(line, committed_by_name[name]) if name in committed_by_name else line)
        merged.extend(anchored.get(name, []))
    kept = sum(len(rows) for rows in anchored.values())
    return merged, kept


def write_surface_tsv(surface: str, out_path: Path) -> None:
    """Write one surface's matrix, preserving the rows the surface owns."""
    fresh = generated_rows(surface)
    committed = (
        out_path.read_text(encoding="utf-8").splitlines() if out_path.is_file() else []
    )
    merged, kept = merge_rows(fresh, committed)
    out_path.write_text("".join(f"{line}\n" for line in merged), encoding="utf-8")
    print(
        f"[{surface}] wrote {len(fresh) - 1} generated scenario(s)"
        + (f", kept {kept} surface-owned row(s)" if kept else "")
        + f" to {out_path.relative_to(ROOT)}"
    )


def matrix_findings(surface: str) -> list[str]:
    """Every generated row must be present in the matrix, in generated order.

    Surface-owned rows are the surface's business, so they are preserved rather
    than judged here; a capture row that nobody produces is caught by the
    receipt freshness guard, which compares the published manifest with the
    matrix that actually ran.
    """
    fresh = generated_rows(surface)
    path = ROOT / f"scripts/capture_{surface}_scenarios.tsv"
    if not path.is_file():
        return [f"{path.relative_to(ROOT)} is missing"]
    committed = path.read_text(encoding="utf-8").splitlines()
    if committed[0].split("\t")[: len(fresh[0].split("\t"))] != fresh[0].split("\t"):
        return [
            f"{path.relative_to(ROOT)} header is {committed[0]!r}, generator writes {fresh[0]!r}"
        ]
    findings: list[str] = []
    position = 1
    for line in fresh[1:]:
        name = line.split("\t")[0]
        while position < len(committed) and committed[position].split("\t")[0] != name:
            position += 1
        if position >= len(committed):
            findings.append(
                f"{path.relative_to(ROOT)} is missing or reorders the generated row {name!r}"
            )
            return findings
        if committed[position] != merge_line(line, committed[position]):
            findings.append(
                f"{path.relative_to(ROOT)} row {name!r} reads {committed[position]!r}, "
                f"generator writes {line!r}"
            )
        position += 1
    return findings


SURFACES = ("slint", "iced", "bevy")


def selected_surfaces(argv: list[str]) -> list[str]:
    chosen = [arg.split("=", 1)[1] for arg in argv if arg.startswith("--surface=")]
    if not chosen:
        return list(SURFACES)
    unknown = [name for name in chosen if name not in SURFACES]
    if unknown:
        raise SystemExit(f"unknown surface(s): {', '.join(unknown)}")
    return chosen


if __name__ == "__main__":
    surfaces = selected_surfaces(sys.argv)
    if "--check" in sys.argv:
        # The generated matrices are committed artifacts. This mode regenerates
        # them in memory and compares, so a hand edit that drops, reorders or
        # rewrites a generated row cannot pass unnoticed.
        failures = [
            finding for surface in surfaces for finding in matrix_findings(surface)
        ]
        for failure in failures:
            print(f"generate_unified_matrix: {failure}", file=sys.stderr)
        if failures:
            sys.exit(1)
        print(
            "generate_unified_matrix: every generated row is present in order for "
            + ", ".join(surfaces)
        )
        sys.exit(0)

    for surface in surfaces:
        write_surface_tsv(surface, ROOT / f"scripts/capture_{surface}_scenarios.tsv")
