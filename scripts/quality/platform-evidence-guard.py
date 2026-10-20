#!/usr/bin/env python3
"""Guard the five-platform build, install and run evidence matrix.

`scripts/platform/evidence/*.json` is the evidence ledger: one file per
platform, each with the four legs `build`, `install`, `run` and `target`.
`docs/PLATFORM_MATRIX.md` carries the generated matrix section, which is a
projection of that ledger.

This guard fails when

  * a ledger is malformed: an unknown leg, an unknown status, a missing
    reason, a malformed evidence identity, an evidence entry that names a
    product the platform does not ship,
  * a leg claims `Proven` without an artifact or a run that covers every
    product of the platform, or claims `Unsupported` although the evidence
    covers every product (a cross-compile check alone never proves a leg),
  * the platform set, the platform labels or the product sets drift away from
    the business coverage matrix of `parity-coverage-guard.py`,
  * a product is not registered in the product identity registry,
  * the generated section of the platform matrix document drifted from the
    ledger.

Usage:
    python3 scripts/quality/platform-evidence-guard.py [--mode report|enforce]
    python3 scripts/quality/platform-evidence-guard.py --self-test

Exit status is 0 when the matrix is consistent, 1 when it is not (in enforce
mode) and 2 when a registry cannot be read.
"""

from __future__ import annotations

import argparse
import importlib.util
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts" / "platform"))
sys.path.insert(0, str(ROOT / "scripts"))

from evidence_model import (  # noqa: E402  (the model lives in scripts/platform)
    LEGS,
    PLATFORM_MATRIX,
    PLATFORM_ORDER,
    STATUS_PROVEN,
    STATUS_UNSUPPORTED,
    PlatformLedger,
    ledger_problems,
    load_ledgers,
    render_section,
    section_span,
)
from product_identity import read_properties  # noqa: E402  (the registry reader lives in scripts/)

PARITY_GUARD = Path("scripts/quality/parity-coverage-guard.py")

# Which identity registry a platform draws its products from.
PLATFORM_SURFACE = {
    "linux": "desktop",
    "windows": "desktop",
    "macos": "desktop",
    "android": "android",
    "ios": "none",
}

TOKEN_PATTERN = re.compile(r"[^a-z0-9]+")


def parity_platform_legs() -> dict[str, tuple[str, ...]]:
    """The platform legs of the business coverage matrix."""
    path = ROOT / PARITY_GUARD
    specification = importlib.util.spec_from_file_location("parity_coverage_guard", path)
    if specification is None or specification.loader is None:
        raise ValueError(f"{path}: the business coverage guard cannot be loaded")
    module = importlib.util.module_from_spec(specification)
    # The guard registers dataclasses; the module must be visible in
    # sys.modules before it is executed.
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return dict(module.PLATFORM_LEGS)


def registered_products() -> dict[str, set[str]]:
    """The registered product ids per identity surface."""
    values = read_properties()
    surfaces: dict[str, set[str]] = {}
    for surface in ("desktop", "android"):
        raw = values.get(f"{surface}.products", "")
        surfaces[surface] = {item.strip() for item in raw.split(",") if item.strip()}
    return surfaces


def document_platform_rows(text: str) -> set[tuple[str, str]]:
    """The platform rows the platform matrix document registers.

    The first row of every table is its header and the second row is the
    separator; neither registers a platform.
    """
    rows: set[tuple[str, str]] = set()
    previous_was_row = False
    for line in text.splitlines():
        if not line.startswith("|"):
            previous_was_row = False
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        is_header = not previous_was_row
        previous_was_row = True
        if is_header or len(cells) < 2:
            continue
        label = cells[0]
        if set(label) <= {"-", " "}:
            continue
        token = TOKEN_PATTERN.split(label.lower())[0]
        if token:
            rows.add((label, token))
    return rows


def ledger_consistency_problems(
    ledger: PlatformLedger,
    parity_legs: dict[str, tuple[str, ...]],
    products: dict[str, set[str]],
) -> list[str]:
    """The rules that tie a ledger to the coverage matrix and the registry."""
    problems: list[str] = []
    surface = PLATFORM_SURFACE.get(ledger.platform)
    if surface is None:
        problems.append(f"{ledger.path}: unknown platform {ledger.platform!r}")
        return problems
    if ledger.platform not in parity_legs:
        problems.append(
            f"{ledger.path}: the platform leg is not registered in the business coverage matrix")
        return problems
    parity_surfaces = set(parity_legs[ledger.platform])
    if surface == "none":
        if ledger.products:
            problems.append(f"{ledger.path}: the platform ships no product, found {ledger.products}")
        if parity_surfaces:
            problems.append(
                f"{ledger.path}: the business coverage matrix expects the surfaces {sorted(parity_surfaces)}")
        return problems
    registered = products.get(surface, set())
    for product in ledger.products:
        if product not in registered:
            problems.append(
                f"{ledger.path}: the product {product!r} is not registered in the identity registry")
    if surface == "desktop":
        if set(ledger.products) != parity_surfaces:
            problems.append(
                f"{ledger.path}: the products {sorted(ledger.products)} differ from the business "
                f"coverage matrix surfaces {sorted(parity_surfaces)}")
    elif not parity_surfaces <= set(ledger.products):
        problems.append(
            f"{ledger.path}: the products {sorted(ledger.products)} do not cover the business "
            f"coverage matrix surfaces {sorted(parity_surfaces)}")
    return problems


def check(root: Path = ROOT) -> tuple[list[str], str]:
    """Audit the ledgers, the document and their mutual consistency."""
    problems: list[str] = []
    ledgers = load_ledgers(root)
    platforms = [ledger.platform for ledger in ledgers]
    if platforms != list(PLATFORM_ORDER):
        missing = sorted(set(PLATFORM_ORDER) - set(platforms))
        extra = sorted(set(platforms) - set(PLATFORM_ORDER))
        problems.append(
            f"the evidence ledgers must cover exactly {list(PLATFORM_ORDER)}: "
            f"missing={missing} unexpected={extra}")
    parity_legs = parity_platform_legs()
    products = registered_products()
    for ledger in ledgers:
        problems.extend(ledger_problems(ledger))
        problems.extend(ledger_consistency_problems(ledger, parity_legs, products))

    document_path = root / PLATFORM_MATRIX
    document = document_path.read_text(encoding="utf-8")
    expected_rows = {(ledger.label, ledger.platform) for ledger in ledgers}
    actual_rows = document_platform_rows(document)
    if expected_rows != actual_rows:
        problems.append(
            "the platform matrix document and the evidence ledgers register different platforms: "
            f"document={sorted(actual_rows)} ledger={sorted(expected_rows)}")
    section = render_section(ledgers)
    try:
        start, end = section_span(document)
    except ValueError as error:
        problems.append(str(error))
    else:
        if document[start:end] != section:
            problems.append(
                "the generated evidence section of the platform matrix document drifted from the "
                "evidence ledgers; run python3 scripts/platform/build-evidence-matrix.py")

    report = render_report(ledgers)
    return problems, report


def render_report(ledgers: list[PlatformLedger]) -> str:
    """The human readable summary of the matrix."""
    lines = ["platform evidence matrix:"]
    for ledger in ledgers:
        lines.append(f"  {ledger.platform:<8} {ledger.label:<16} {ledger.verdict()}")
        for leg in ledger.legs:
            covered = sorted(leg.covered_products())
            lines.append(
                f"      {leg.name:<8} {leg.status:<12} covered={','.join(covered) or '-'}")
    return "\n".join(lines)


def synthetic_ledger(platform: str = "linux") -> PlatformLedger:
    """A ledger that satisfies every rule of the guard."""
    from evidence_model import Evidence, Leg

    products = ("iced", "bevy", "slint")
    evidence = tuple(
        Evidence(
            kind="artifact",
            product=product,
            identity="sha256:" + chr(ord("a") + index) * 64,
            detail=f"dist/infiltrator_0.1.0_amd64.deb ({product})",
        )
        for index, product in enumerate(products)
    )
    legs = []
    for name in LEGS:
        legs.append(Leg(
            name=name,
            status=STATUS_PROVEN,
            reason="",
            command="bash packaging/linux/build-linux.sh",
            evidence=evidence,
        ))
    return PlatformLedger(
        platform=platform,
        label="Linux desktop",
        products=products,
        legs=tuple(legs),
        path=Path("scripts/platform/evidence/linux.json"),
    )


def self_test() -> int:
    """Exercise every failure mode of the guard."""
    from evidence_model import Evidence, Leg

    ledger = synthetic_ledger()
    assert ledger_problems(ledger) == [], ledger_problems(ledger)

    def problems_of(mutate) -> list[str]:
        candidate = mutate(ledger)
        return ledger_problems(candidate)

    def replace_leg(mutated: PlatformLedger, name: str, **fields) -> PlatformLedger:
        legs = tuple(
            Leg(
                name=leg.name,
                status=fields.get("status", leg.status),
                reason=fields.get("reason", leg.reason),
                command=fields.get("command", leg.command),
                evidence=fields.get("evidence", leg.evidence),
            )
            if leg.name == name else leg
            for leg in mutated.legs
        )
        return PlatformLedger(
            platform=mutated.platform,
            label=mutated.label,
            products=mutated.products,
            legs=legs,
            path=mutated.path,
        )

    unknown_status = problems_of(
        lambda item: replace_leg(item, "build", status="maybe"))
    assert any("unknown status" in problem for problem in unknown_status), unknown_status

    missing_reason = problems_of(
        lambda item: replace_leg(item, "build", status=STATUS_UNSUPPORTED, reason=""))
    assert any("unsupported leg needs a reason" in problem for problem in missing_reason), \
        missing_reason

    cross_compile_only = problems_of(
        lambda item: replace_leg(item, "build", evidence=(
            Evidence(
                kind="cross_compile",
                product="",
                identity="command:bash scripts/cross-check.sh",
                detail="the crates only cross-compile",
            ),
        )))
    assert any("claims Proven" in problem for problem in cross_compile_only), cross_compile_only

    bad_identity = problems_of(
        lambda item: replace_leg(item, "build", evidence=(
            Evidence(
                kind="artifact",
                product="iced",
                identity="not-a-digest",
                detail="dist/infiltrator_0.1.0_amd64.deb",
            ),
        )))
    assert any("sha256:" in problem for problem in bad_identity), bad_identity

    unknown_product = problems_of(
        lambda item: replace_leg(item, "build", evidence=(
            Evidence(
                kind="artifact",
                product="compose",
                identity="sha256:" + "b" * 64,
                detail="dist/infiltrator_0.1.0_amd64.deb",
            ),
        )))
    assert any("unregistered product" in problem for problem in unknown_product), unknown_product

    missing_leg = problems_of(
        lambda item: PlatformLedger(
            platform=item.platform,
            label=item.label,
            products=item.products,
            legs=item.legs[:2],
            path=item.path,
        ))
    assert any("the legs must be exactly" in problem for problem in missing_leg), missing_leg

    unsupported_with_full_coverage = problems_of(
        lambda item: replace_leg(item, "build", status=STATUS_UNSUPPORTED,
                                  reason="not built yet"))
    assert any("claims Unsupported" in problem
               for problem in unsupported_with_full_coverage), unsupported_with_full_coverage

    parity_legs = parity_platform_legs()
    products = registered_products()
    assert set(parity_legs) == set(PLATFORM_ORDER), parity_legs
    assert ledger_consistency_problems(ledger, parity_legs, products) == []

    drifted_products = PlatformLedger(
        platform=ledger.platform,
        label=ledger.label,
        products=("iced", "bevy"),
        legs=ledger.legs,
        path=ledger.path,
    )
    consistency = ledger_consistency_problems(drifted_products, parity_legs, products)
    assert any("differ from the business coverage matrix" in problem for problem in consistency), \
        consistency

    unknown_platform = PlatformLedger(
        platform="solaris",
        label="Solaris",
        products=(),
        legs=ledger.legs,
        path=ledger.path,
    )
    assert any("unknown platform" in problem
               for problem in ledger_consistency_problems(unknown_platform, parity_legs, products))

    print("platform evidence guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()

    try:
        problems, report = check()
    except (OSError, ValueError) as error:
        print(f"platform evidence guard: cannot read the evidence ledgers: {error}",
              file=sys.stderr)
        return 2

    print(report)
    if problems:
        print(f"platform evidence guard: FAIL ({len(problems)} problems)", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    print("platform evidence guard: OK (every platform leg carries its evidence identity)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
