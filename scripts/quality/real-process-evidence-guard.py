#!/usr/bin/env python3
"""Guard the real-binary evidence of the two engine drivers (PUB-006.8).

The neutral `mihomo-driver` and `singbox-driver` crates are only worth
publishing when they are proven against a real engine process. The proof lives
in a named test target per driver, and this guard keeps that proof honest:

1. The named entry point exists per driver
   (`crates/<driver>/tests/real_process_evidence.rs`) and stays `#[ignore]`d,
   so the ordinary hermetic gate never starts an engine.
2. The entry point is reachable: every local smoke entry point runs it with
   `--run-ignored`, and the environment contract lives in the driver, so the
   script and the test can never disagree about it.
3. The environment contract stays typed: one enum with the mutually exclusive
   `Absent` / `Present` / `Invalid` outcomes, never a combination of flags.
4. An absent environment produces an honest `environment_absent` receipt and
   never a silent pass; the receipt vocabulary is an enum, too.
5. All four required proofs are declared and implemented: the controller
   handshake, the configuration write, the node speed test and the traffic
   dispatch.
6. The smoke entry points print a machine-readable summary and never hardcode
   an environment key that the typed contract owns.

Usage
-----
    python3 scripts/quality/real-process-evidence-guard.py
    python3 scripts/quality/real-process-evidence-guard.py --self-test
"""

from __future__ import annotations

import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# The driver crates and the environment prefix their contract owns.
DRIVERS = (
    ("mihomo-driver", "MIHOMO_REAL_"),
    ("singbox-driver", "SINGBOX_REAL_"),
)

# The four capabilities the real-binary evidence must prove. Every entry
# names the typed variant and its machine-readable name.
PROOFS = (
    ("ApiHandshake", "api_handshake"),
    ("ConfigurationWrite", "configuration_write"),
    ("NodeSpeedTest", "node_speed_test"),
    ("TrafficDispatch", "traffic_dispatch"),
)

# The mutually exclusive outcomes of the environment contract.
ENVIRONMENT_VARIANTS = ("Absent", "Present", "Invalid")

# The mutually exclusive outcomes of the evidence receipt.
RECEIPT_VARIANTS = (
    "EnvironmentAbsent",
    "InvalidEnvironment",
    "Complete",
    "UnsupportedProof",
    "Defect",
)

# The local entry points that must run the named evidence target.
SMOKE_SCRIPTS = (
    "scripts/core-backend-smoke-mihomo.sh",
    "scripts/core-backend-smoke-singbox.sh",
    "scripts/core-backend-smoke-conformance.sh",
)

# The machine-readable summary marker every entry point prints.
SUMMARY_MARKER = "MUSIC_FROG_SMOKE_SUMMARY"

# The environment key constants the typed contract owns.
ENV_CONSTANT = re.compile(r'pub const ([A-Z0-9_]+_ENV): &str = "([A-Z0-9_]+)";')

# A proof variant of the receipt vocabulary.
PROOF_VARIANT = re.compile(r"^\s{4}([A-Z][A-Za-z0-9]*),$", re.MULTILINE)


def driver_root(root: Path, crate: str) -> Path:
    return root / "crates" / crate


def check_driver(root: Path, crate: str, prefix: str) -> list[str]:
    """Audits one driver crate."""
    problems: list[str] = []
    base = driver_root(root, crate)
    target = base / "tests" / "real_process_evidence.rs"
    contract = base / "tests" / "real_process_evidence" / "contract.rs"
    receipt = base / "tests" / "real_process_evidence" / "receipt.rs"
    proofs = base / "tests" / "real_process_evidence" / "proofs.rs"

    for path in (target, contract, receipt, proofs):
        if not path.is_file():
            problems.append(f"{path}: the real-binary evidence source is missing")
    if problems:
        return problems

    target_text = target.read_text(encoding="utf-8")
    contract_text = contract.read_text(encoding="utf-8")
    receipt_text = receipt.read_text(encoding="utf-8")
    proofs_text = proofs.read_text(encoding="utf-8")

    # 1. The named entry point stays ignored so the hermetic gate is hermetic.
    if "#[ignore" not in target_text:
        problems.append(
            f"{target}: the real-binary evidence target must stay `#[ignore]`d"
        )
    if "#[tokio::test]" not in target_text:
        problems.append(f"{target}: the real-binary evidence target must be an async test")

    # 3. The environment contract is one typed enum with exclusive outcomes.
    if "pub enum RealProcessEnvironment" not in contract_text:
        problems.append(
            f"{contract}: the environment contract must be a typed enum"
        )
    for variant in ENVIRONMENT_VARIANTS:
        if not re.search(rf"^\s+{variant}\b", contract_text, re.MULTILINE):
            problems.append(
                f"{contract}: the environment contract must declare the `{variant}` outcome"
            )
    for constant in ("REQUIRED_ENV_KEYS", "OPTIONAL_ENV_KEYS"):
        if f"pub const {constant}" not in contract_text:
            problems.append(f"{contract}: the contract must declare `{constant}`")
    constants = ENV_CONSTANT.findall(contract_text)
    if not constants:
        problems.append(f"{contract}: the contract must declare its environment keys")
    for name, value in constants:
        if not value.startswith(prefix):
            problems.append(
                f"{contract}: `{name}` names `{value}`, which is outside the `{prefix}` namespace"
            )

    # 4. The receipt vocabulary is typed and knows the absent environment.
    if "pub enum EvidenceOutcome" not in receipt_text:
        problems.append(f"{receipt}: the evidence outcome must be a typed enum")
    for variant in RECEIPT_VARIANTS:
        if not re.search(rf"^\s+{variant}\b", receipt_text, re.MULTILINE):
            problems.append(
                f"{receipt}: the evidence outcome must declare the `{variant}` state"
            )
    if "pub enum Proof" not in receipt_text:
        problems.append(f"{receipt}: the proof vocabulary must be a typed enum")
    for variant, proof in PROOFS:
        if not re.search(rf"^\s+{variant}\b", receipt_text, re.MULTILINE):
            problems.append(f"{receipt}: the proof vocabulary must declare `{variant}`")
        if f'"{proof}"' not in receipt_text:
            problems.append(f"{receipt}: the proof vocabulary must name `{proof}`")
    if "pub const ALL: [Proof; 4]" not in receipt_text:
        problems.append(f"{receipt}: the proof vocabulary must declare every required proof")

    # 5. Every declared proof is implemented by the proof module.
    for variant, proof in PROOFS:
        if f"Proof::{variant}" not in proofs_text:
            problems.append(
                f"{proofs}: the proof module must run the `{proof}` proof"
            )

    # 4. The absent environment is reported, never silently green.
    if "RealProcessEnvironment::Absent" not in target_text:
        problems.append(
            f"{target}: the entry point must handle the absent environment explicitly"
        )
    if "environment_absent" not in receipt_text:
        problems.append(
            f"{receipt}: the receipt must name the `environment_absent` outcome"
        )
    if "environment_absent" not in target_text:
        problems.append(
            f"{target}: the entry point must publish the `environment_absent` receipt"
        )
    if "publish" not in target_text:
        problems.append(f"{target}: the entry point must publish its machine-readable receipt")
    return problems


def contract_keys(root: Path, crate: str) -> list[tuple[str, str]]:
    """Returns the environment key constants the typed contract owns."""
    path = (
        root
        / "crates"
        / crate
        / "tests"
        / "real_process_evidence"
        / "contract.rs"
    )
    if not path.is_file():
        return []
    return ENV_CONSTANT.findall(path.read_text(encoding="utf-8"))


def check_smoke_scripts(root: Path) -> list[str]:
    """Audits the local entry points that run the named evidence target."""
    problems: list[str] = []
    engine_scripts = (
        "scripts/core-backend-smoke-mihomo.sh",
        "scripts/core-backend-smoke-singbox.sh",
    )
    for relative in SMOKE_SCRIPTS:
        path = root / relative
        if not path.is_file():
            problems.append(f"{path}: the smoke entry point is missing")
            continue
        text = path.read_text(encoding="utf-8")
        if SUMMARY_MARKER not in text:
            problems.append(
                f"{path}: the smoke entry point must print the `{SUMMARY_MARKER}` summary"
            )
        if relative in engine_scripts:
            if "--test real_process_evidence" not in text:
                problems.append(
                    f"{path}: the smoke entry point must run the named evidence target"
                )
            if "--run-ignored" not in text:
                problems.append(
                    f"{path}: the smoke entry point must request the ignored evidence target"
                )
            if "real_process_evidence/contract.rs" not in text:
                problems.append(
                    f"{path}: the smoke entry point must read the environment contract "
                    "of the driver"
                )
            if not re.search(r"pub const \(\[A-Z0-9_\]\+\)", text):
                problems.append(
                    f"{path}: the smoke entry point must derive the environment keys "
                    "from the contract"
                )
            for name, value in contract_keys(root, "mihomo-driver") + contract_keys(
                root, "singbox-driver"
            ):
                if name == "BINARY_ENV":
                    # The binary key is also the product-level smoke input, so
                    # the entry points may name it.
                    continue
                if f"{value}=" in text:
                    problems.append(
                        f"{path}: the environment key `{value}` belongs to the typed "
                        "contract and must not be hardcoded in the entry point"
                    )
        else:
            for engine_script in engine_scripts:
                if engine_script.split("/")[-1] not in text:
                    problems.append(
                        f"{path}: the conformance entry point must delegate to "
                        f"{engine_script}"
                    )
            if "--driver-evidence-only" not in text:
                problems.append(
                    f"{path}: the conformance entry point must request the driver evidence"
                )
            for crate in ("mihomo-driver", "singbox-driver"):
                binary_key = next(
                    (
                        value
                        for name, value in contract_keys(root, crate)
                        if name == "BINARY_ENV"
                    ),
                    "",
                )
                if binary_key and f"{binary_key}=" not in text:
                    problems.append(
                        f"{path}: the conformance entry point must hand the binary over "
                        f"through the contract key `{binary_key}`"
                    )
    return problems


def check(root: Path = ROOT) -> list[str]:
    """Runs every audit and returns the problems it found."""
    problems: list[str] = []
    for crate, prefix in DRIVERS:
        problems.extend(check_driver(root, crate, prefix))
    problems.extend(check_smoke_scripts(root))
    return problems


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _synthetic_tree(root: Path) -> None:
    """Builds a minimal tree that satisfies every audit."""
    for crate, prefix in DRIVERS:
        base = root / "crates" / crate / "tests" / "real_process_evidence"
        _write(
            base / "contract.rs",
            "\n".join(
                [
                    "pub const BINARY_ENV: &str = \""
                    + prefix
                    + "BINARY\";",
                    "pub const CONFIG_ENV: &str = \""
                    + prefix
                    + "CONFIG\";",
                    "pub const REQUIRED_ENV_KEYS: [&str; 2] = [BINARY_ENV, CONFIG_ENV];",
                    "pub const OPTIONAL_ENV_KEYS: [&str; 0] = [];",
                    "pub enum RealProcessEnvironment {",
                    "    Absent { missing: Vec<&'static str> },",
                    "    Present(RealProcessTarget),",
                    "    Invalid { reason: String },",
                    "}",
                    "",
                ]
            ),
        )
        _write(
            base / "receipt.rs",
            "\n".join(
                [
                    "pub enum Proof {",
                    "    ApiHandshake,",
                    "    ConfigurationWrite,",
                    "    NodeSpeedTest,",
                    "    TrafficDispatch,",
                    "}",
                    "impl Proof {",
                    "    pub const ALL: [Proof; 4] = [",
                    "        Proof::ApiHandshake,",
                    "        Proof::ConfigurationWrite,",
                    "        Proof::NodeSpeedTest,",
                    "        Proof::TrafficDispatch,",
                    "    ];",
                    "    pub const fn as_str(self) -> &'static str {",
                    '        match self {',
                    '            Self::ApiHandshake => "api_handshake",',
                    '            Self::ConfigurationWrite => "configuration_write",',
                    '            Self::NodeSpeedTest => "node_speed_test",',
                    '            Self::TrafficDispatch => "traffic_dispatch",',
                    "        }",
                    "    }",
                    "}",
                    "pub enum EvidenceOutcome {",
                    "    EnvironmentAbsent { missing: Vec<String> },",
                    "    InvalidEnvironment { reason: String },",
                    "    Complete,",
                    "    UnsupportedProof { proof: Proof },",
                    "    Defect { reason: String },",
                    "}",
                    'pub const ABSENT_KIND: &str = "environment_absent";',
                    "",
                ]
            ),
        )
        _write(
            base / "proofs.rs",
            "\n".join(
                [
                    "pub async fn run() {",
                    "    let _ = Proof::ApiHandshake;",
                    "    let _ = Proof::ConfigurationWrite;",
                    "    let _ = Proof::NodeSpeedTest;",
                    "    let _ = Proof::TrafficDispatch;",
                    "}",
                    "",
                ]
            ),
        )
        _write(
            root / "crates" / crate / "tests" / "real_process_evidence.rs",
            "\n".join(
                [
                    "#[tokio::test]",
                    "#[ignore = \"requires the real binary\"]",
                    "async fn real_process_evidence() {",
                    "    match contract::resolve() {",
                    "        RealProcessEnvironment::Absent { .. } => {",
                    "            publish(\"environment_absent\");",
                    "        }",
                    "        _ => {}",
                    "    }",
                    "}",
                    "",
                ]
            ),
        )
    for relative in SMOKE_SCRIPTS:
        if relative == "scripts/core-backend-smoke-conformance.sh":
            _write(
                root / relative,
                "\n".join(
                    [
                        "#!/usr/bin/env bash",
                        "bash scripts/core-backend-smoke-mihomo.sh --driver-evidence-only",
                        "bash scripts/core-backend-smoke-singbox.sh --driver-evidence-only",
                        "MIHOMO_REAL_BINARY=/tmp/mihomo",
                        "SINGBOX_REAL_BINARY=/tmp/sing-box",
                        f"printf '{SUMMARY_MARKER} {{}}\\n'",
                        "",
                    ]
                ),
            )
            continue
        _write(
            root / relative,
            "\n".join(
                [
                    "#!/usr/bin/env bash",
                    "contract='crates/mihomo-driver/tests/real_process_evidence/contract.rs'",
                    "python3 - \"$contract\" <<'PY'",
                    "import re",
                    "print(re.findall(r'pub const ([A-Z0-9_]+)', text))",
                    "PY",
                    "cargo nextest run -p mihomo-driver --test real_process_evidence --run-ignored only",
                    f"printf '{SUMMARY_MARKER} {{}}\\n'",
                    "",
                ]
            ),
        )


def self_test() -> int:
    """Proves the gate accepts the healthy shape and rejects every rot."""
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        _synthetic_tree(root)
        problems = check(root)
        assert not problems, problems

        # The evidence target must not rot back into an unignored test.
        target = root / "crates" / "mihomo-driver" / "tests" / "real_process_evidence.rs"
        original = target.read_text(encoding="utf-8")
        target.write_text(
            original.replace('#[ignore = "requires the real binary"]', "// ignored"),
            encoding="utf-8",
        )
        problems = check(root)
        assert any("`#[ignore]`d" in problem for problem in problems), problems
        target.write_text(original, encoding="utf-8")

        # The environment contract must stay typed.
        contract = (
            root / "crates" / "singbox-driver" / "tests" / "real_process_evidence" / "contract.rs"
        )
        original = contract.read_text(encoding="utf-8")
        contract.write_text(
            original.replace("pub enum RealProcessEnvironment {", "pub struct RealProcessEnvironment {"),
            encoding="utf-8",
        )
        problems = check(root)
        assert any("must be a typed enum" in problem for problem in problems), problems
        contract.write_text(original, encoding="utf-8")

        # The absent environment must stay an explicit outcome.
        original = contract.read_text(encoding="utf-8")
        contract.write_text(
            original.replace("    Absent { missing: Vec<&'static str> },\n", ""),
            encoding="utf-8",
        )
        problems = check(root)
        assert any("`Absent` outcome" in problem for problem in problems), problems
        contract.write_text(original, encoding="utf-8")

        # A required proof must not disappear from the vocabulary.
        receipt = (
            root / "crates" / "mihomo-driver" / "tests" / "real_process_evidence" / "receipt.rs"
        )
        original = receipt.read_text(encoding="utf-8")
        receipt.write_text(
            original.replace("    NodeSpeedTest,\n", ""), encoding="utf-8"
        )
        problems = check(root)
        assert any("`NodeSpeedTest`" in problem for problem in problems), problems
        receipt.write_text(original, encoding="utf-8")

        # A proof must not stop being implemented.
        proofs = (
            root / "crates" / "singbox-driver" / "tests" / "real_process_evidence" / "proofs.rs"
        )
        original = proofs.read_text(encoding="utf-8")
        proofs.write_text(
            original.replace("    let _ = Proof::TrafficDispatch;\n", ""), encoding="utf-8"
        )
        problems = check(root)
        assert any("traffic_dispatch" in problem for problem in problems), problems
        proofs.write_text(original, encoding="utf-8")

        # The named entry point must stay reachable from the smoke scripts.
        smoke = root / SMOKE_SCRIPTS[0]
        original = smoke.read_text(encoding="utf-8")
        smoke.write_text(
            original.replace("--test real_process_evidence", "--test other"), encoding="utf-8"
        )
        problems = check(root)
        assert any("must run the named evidence target" in problem for problem in problems), problems
        smoke.write_text(original, encoding="utf-8")

        # The machine-readable summary must stay.
        smoke.write_text(
            original.replace(SUMMARY_MARKER, "SUMMARY"), encoding="utf-8"
        )
        problems = check(root)
        assert any("summary" in problem for problem in problems), problems
        smoke.write_text(original, encoding="utf-8")

        # The environment keys belong to the typed contract.
        smoke.write_text(
            original + "\nMIHOMO_REAL_CONFIG=/tmp/mihomo-config\n", encoding="utf-8"
        )
        problems = check(root)
        assert any("must not be hardcoded" in problem for problem in problems), problems
        smoke.write_text(original, encoding="utf-8")

    print("real-process evidence guard self-test: PASS")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()
    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1
    print("real-process evidence guard: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
