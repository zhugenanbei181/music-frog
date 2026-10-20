#!/usr/bin/env python3
"""The platform evidence model: the ledger, its rules and its rendering.

The platform evidence ledger (`scripts/platform/evidence/<platform>.json`) is
the single source of truth for the build, install, run and target evidence of
every platform. The generated section of `docs/PLATFORM_MATRIX.md` is a
projection of that ledger and never a second source: the guard re-renders the
section and fails when it drifts.

The model is typed on purpose:

* a leg is one of `build`, `install`, `run`, `target`; there is no leg list
  that grows a second meaning,
* a leg status is either `Proven` or `Unsupported`; it is never a pair of
  booleans,
* an evidence entry is an `artifact`, a `run` or a `cross_compile` check, and
  only the first two can carry a `Proven` leg on their own. A cross-compile
  check is never enough to prove that a platform builds, installs or runs.

Usage:
    python3 scripts/platform/evidence_model.py --check
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE_DIR = Path("scripts/platform/evidence")
PLATFORM_MATRIX = Path("docs/PLATFORM_MATRIX.md")

LEG_BUILD = "build"
LEG_INSTALL = "install"
LEG_RUN = "run"
LEG_TARGET = "target"
LEGS = (LEG_BUILD, LEG_INSTALL, LEG_RUN, LEG_TARGET)

STATUS_PROVEN = "Proven"
STATUS_UNSUPPORTED = "Unsupported"
STATUSES = (STATUS_PROVEN, STATUS_UNSUPPORTED)

KIND_ARTIFACT = "artifact"
KIND_RUN = "run"
KIND_CROSS_COMPILE = "cross_compile"
KINDS = (KIND_ARTIFACT, KIND_RUN, KIND_CROSS_COMPILE)
# The evidence kinds that can carry a proven leg on their own.
SUFFICIENT_KINDS = (KIND_ARTIFACT, KIND_RUN)

# The platform order of the matrix; it matches the order of the business
# coverage matrix legs.
PLATFORM_ORDER = ("linux", "windows", "macos", "android", "ios")

SECTION_START = "<!-- platform-evidence:start -->"
SECTION_END = "<!-- platform-evidence:end -->"

SHA256_PATTERN = re.compile(r"^sha256:[0-9a-f]{64}$")
RUN_ID_PATTERN = re.compile(r"^run:[A-Za-z0-9][A-Za-z0-9._:+-]*$")
COMMAND_PATTERN = re.compile(r"^command:\S.*$")


@dataclass(frozen=True)
class Evidence:
    """One evidence fact about a platform leg."""

    kind: str
    product: str
    identity: str
    detail: str

    def as_dict(self) -> dict[str, str]:
        return {
            "kind": self.kind,
            "product": self.product,
            "identity": self.identity,
            "detail": self.detail,
        }


@dataclass(frozen=True)
class Leg:
    """One leg of the platform evidence matrix."""

    name: str
    status: str
    reason: str
    command: str
    evidence: tuple[Evidence, ...]

    def covered_products(self) -> set[str]:
        """The products a sufficient evidence entry covers."""
        return {
            entry.product
            for entry in self.evidence
            if entry.kind in SUFFICIENT_KINDS and entry.product
        }

    def derived_status(self, products: tuple[str, ...]) -> str:
        """The status the evidence supports, whatever the ledger claims."""
        if not products:
            return STATUS_UNSUPPORTED
        if self.covered_products() >= set(products):
            return STATUS_PROVEN
        return STATUS_UNSUPPORTED


@dataclass(frozen=True)
class PlatformLedger:
    """The evidence ledger of one platform."""

    platform: str
    label: str
    products: tuple[str, ...]
    legs: tuple[Leg, ...]
    path: Path

    def leg(self, name: str) -> Leg | None:
        for leg in self.legs:
            if leg.name == name:
                return leg
        return None

    def verdict(self) -> str:
        """The platform conclusion derived from its legs."""
        statuses = [leg.derived_status(self.products) for leg in self.legs]
        if statuses and all(status == STATUS_PROVEN for status in statuses):
            return STATUS_PROVEN
        if any(status == STATUS_PROVEN for status in statuses):
            return "Partial"
        return STATUS_UNSUPPORTED


def parse_ledger(path: Path) -> PlatformLedger:
    """Read one ledger file."""
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path}: the ledger must be a JSON object")
    legs: list[Leg] = []
    for entry in payload.get("legs", []):
        evidence = tuple(
            Evidence(
                kind=item.get("kind", ""),
                product=item.get("product", ""),
                identity=item.get("identity", ""),
                detail=item.get("detail", ""),
            )
            for item in entry.get("evidence", [])
        )
        legs.append(Leg(
            name=entry.get("leg", ""),
            status=entry.get("status", ""),
            reason=entry.get("reason", ""),
            command=entry.get("command", ""),
            evidence=evidence,
        ))
    return PlatformLedger(
        platform=payload.get("platform", ""),
        label=payload.get("label", ""),
        products=tuple(payload.get("products", [])),
        legs=tuple(legs),
        path=path,
    )


def load_ledgers(root: Path = ROOT) -> list[PlatformLedger]:
    """Read every ledger of the evidence directory, in the matrix order."""
    directory = root / EVIDENCE_DIR
    ledgers = []
    for platform in PLATFORM_ORDER:
        path = directory / f"{platform}.json"
        if not path.is_file():
            continue
        ledgers.append(parse_ledger(path))
    return ledgers


def identity_problem(entry: Evidence) -> str | None:
    """The rule an evidence identity must satisfy, or None."""
    if entry.kind == KIND_ARTIFACT:
        if not SHA256_PATTERN.fullmatch(entry.identity):
            return f"an artifact identity must be sha256:<64 hex>, found {entry.identity!r}"
        return None
    if entry.kind == KIND_RUN:
        if not RUN_ID_PATTERN.fullmatch(entry.identity):
            return f"a run identity must be run:<id>, found {entry.identity!r}"
        return None
    if entry.kind == KIND_CROSS_COMPILE:
        if not COMMAND_PATTERN.fullmatch(entry.identity):
            return f"a cross-compile identity must be command:<command>, found {entry.identity!r}"
        return None
    return f"unknown evidence kind {entry.kind!r}"


def leg_problems(leg: Leg, products: tuple[str, ...]) -> list[str]:
    """The rules one leg must satisfy."""
    problems: list[str] = []
    if leg.status not in STATUSES:
        problems.append(f"leg {leg.name}: unknown status {leg.status!r}")
    if not leg.command.strip():
        problems.append(f"leg {leg.name}: the closing command is missing")
    for entry in leg.evidence:
        detail = identity_problem(entry)
        if detail is not None:
            problems.append(f"leg {leg.name}: {detail}")
        if not entry.detail.strip():
            problems.append(f"leg {leg.name}: the evidence {entry.identity!r} has no detail")
        if entry.product and entry.product not in products:
            problems.append(
                f"leg {leg.name}: the evidence names the unregistered product {entry.product!r}")
    derived = leg.derived_status(products)
    if leg.status == STATUS_PROVEN and derived != STATUS_PROVEN:
        missing = sorted(set(products) - leg.covered_products())
        problems.append(
            f"leg {leg.name}: the status claims {STATUS_PROVEN} but the evidence does not cover "
            f"{missing or 'the platform products'}"
        )
    if leg.status == STATUS_UNSUPPORTED and derived == STATUS_PROVEN:
        problems.append(
            f"leg {leg.name}: the status claims {STATUS_UNSUPPORTED} but the evidence covers every product")
    if leg.status == STATUS_UNSUPPORTED and not leg.reason.strip():
        problems.append(f"leg {leg.name}: an unsupported leg needs a reason")
    if leg.status == STATUS_PROVEN and leg.reason.strip():
        problems.append(f"leg {leg.name}: a proven leg must not carry an unsupported reason")
    return problems


def ledger_problems(ledger: PlatformLedger) -> list[str]:
    """The rules one platform ledger must satisfy."""
    problems: list[str] = []
    if ledger.platform != ledger.path.stem:
        problems.append(
            f"{ledger.path}: the ledger declares the platform {ledger.platform!r}")
    if not ledger.label.strip():
        problems.append(f"{ledger.path}: the platform label is missing")
    names = [leg.name for leg in ledger.legs]
    if names != list(LEGS):
        problems.append(f"{ledger.path}: the legs must be exactly {list(LEGS)}, found {names}")
    for leg in ledger.legs:
        problems.extend(leg_problems(leg, ledger.products))
    return problems


def _cell(leg: Leg) -> str:
    """Render one matrix cell."""
    identities = "；".join(
        f"{entry.product or '平台'}：{entry.identity}" for entry in leg.evidence
    )
    parts = [f"**{leg.status}**"]
    parts.append(f"证据：{identities}" if identities else "证据：无")
    if leg.status == STATUS_UNSUPPORTED and leg.reason:
        parts.append(f"原因：{leg.reason}")
    parts.append(f"命令：`{leg.command}`")
    return " · ".join(parts)


def render_section(ledgers: list[PlatformLedger]) -> str:
    """Render the generated matrix section."""
    lines = [
        SECTION_START,
        "## 构建-安装-运行证据矩阵（生成）",
        "",
        "本节由 `scripts/platform/build-evidence-matrix.py` 从证据登记册 "
        "`scripts/platform/evidence/*.json` 生成，登记册是唯一真源；"
        "`scripts/quality/platform-evidence-guard.py` 校验本节与登记册逐字一致。",
        "",
        "每一格给出状态、证据身份（产物 SHA-256、run id 或命令）与闭合该腿的命令；"
        "`Unsupported` 格给出原因与阻塞方。交叉编译检查（`cross_compile`）"
        "不能单独证明任何一腿，因此带交叉编译的腿仍是 `Unsupported`。",
        "",
        "| 平台 | 构建 | 安装 | 运行 | 目标机 / 设备 | 平台结论 |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for ledger in ledgers:
        cells = []
        for name in LEGS:
            leg = ledger.leg(name)
            cells.append(_cell(leg) if leg is not None else "**Unsupported** · 原因：腿缺失")
        lines.append(
            f"| {ledger.label} | " + " | ".join(cells) + f" | {ledger.verdict()} |")
    lines.append("")
    lines.append(SECTION_END)
    return "\n".join(lines)


def section_span(text: str) -> tuple[int, int]:
    """The character span of the generated section inside the document."""
    start = text.find(SECTION_START)
    end = text.find(SECTION_END)
    if start < 0 or end < 0 or end < start:
        raise ValueError("the platform matrix document has no generated evidence section")
    return start, end + len(SECTION_END)


def replace_section(text: str, section: str) -> str:
    """Replace the generated section inside the document."""
    start, end = section_span(text)
    return text[:start] + section + text[end:]


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="only report the drift")
    arguments = parser.parse_args()
    ledgers = load_ledgers()
    problems: list[str] = []
    for ledger in ledgers:
        problems.extend(ledger_problems(ledger))
    section = render_section(ledgers)
    document = (ROOT / PLATFORM_MATRIX).read_text(encoding="utf-8")
    try:
        start, end = section_span(document)
    except ValueError as error:
        problems.append(str(error))
        start, end = 0, 0
    if start and document[start:end] != section:
        problems.append("the generated evidence section drifted from the ledger")
    if problems:
        for problem in problems:
            print(f"platform evidence model: {problem}")
        return 1
    print("platform evidence model: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
