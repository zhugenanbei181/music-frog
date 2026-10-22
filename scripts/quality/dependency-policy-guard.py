#!/usr/bin/env python3
"""Keep the enforced dependency policy consistent with its disclosure source.

`THIRD-PARTY-NOTICES.md` is the disclosure truth: it says which licenses ship
with the repository and why. `deny.toml` is the enforcement truth for the cargo
graph. This guard is the join between them and additionally proves that CI runs
`cargo deny check` from a tag-gated job, so the policy cannot rot unseen.

The guard is offline: it parses TOML and the workflow text, it never resolves
the dependency graph. `cargo deny check` itself runs in the ecosystem gate.
"""

from __future__ import annotations

import argparse
import importlib.util
from pathlib import Path
import re
import sys
import tomllib
import unittest

REPO_ROOT = Path(__file__).resolve().parents[2]
POLICY = Path("deny.toml")
NOTICES = Path("THIRD-PARTY-NOTICES.md")
ECOSYSTEM_WORKFLOW = Path(".github/workflows/ecosystem-gate.yml")

CRATES_IO_INDEX = "https://github.com/rust-lang/crates.io-index"
TAG_JOB_CONDITION = "    if: github.ref_type == 'tag' && startsWith(github.ref_name, 'v')"
DENY_COMMAND = "cargo deny check"

# The disclosure file spells two licenses the way upstream does; cargo-deny
# requires the SPDX identifier. The mapping is deliberately tiny and explicit.
NOTICES_TO_SPDX = {
    "MIT": "MIT",
    "Apache-2.0": "Apache-2.0",
    "BSD-2-Clause": "BSD-2-Clause",
    "BSD-3-Clause": "BSD-3-Clause",
    "ISC": "ISC",
    "Unlicense": "Unlicense",
    "CC0-1.0": "CC0-1.0",
    "Zlib": "Zlib",
    "MPL-2.0": "MPL-2.0",
    "SIL OFL 1.1": "OFL-1.1",
    "OFL-1.1": "OFL-1.1",
    "CC-BY-4.0": "CC-BY-4.0",
    "GPL-3.0": "GPL-3.0",
}
# The mihomo binaries in `vendor/` are redistributed artifacts, not cargo
# dependencies, so their GPL-3.0 never belongs in the cargo allow list.
VENDORED_ONLY = {"GPL-3.0"}
# Licenses cargo-deny may be asked to allow. Every one is permissive or weak
# copyleft; the list exists so a typo in `deny.toml` cannot silently disable a
# check by matching nothing.
KNOWN_SPDX = {
    "0BSD",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-1-Clause",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "CC0-1.0",
    "CDLA-Permissive-2.0",
    "ISC",
    "MIT",
    "MIT-0",
    "MPL-2.0",
    "NCSA",
    "OFL-1.1",
    "Unicode-3.0",
    "Unlicense",
    "WTFPL",
    "Zlib",
    "bzip2-1.0.6",
}
COPYLEFT = re.compile(r"^(?:A?GPL|SSPL|BUSL|Elastic|Commons-Clause)", re.IGNORECASE)
LICENSE_GUARD = Path("scripts/quality/license-guard.py")


def _load_license_guard() -> object:
    """Reuse the notices parser so the license vocabulary has one owner."""
    spec = importlib.util.spec_from_file_location(
        "license_guard_for_dependency_policy",
        Path(__file__).resolve().parent / "license-guard.py",
    )
    if spec is None or spec.loader is None:  # pragma: no cover - defensive
        raise RuntimeError(f"cannot load {LICENSE_GUARD}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _workflow_job_containing(text: str, needle: str) -> str | None:
    """Return the `  <job>:` block that contains `needle`, if any."""
    lines = text.splitlines()
    matches = [index for index, line in enumerate(lines) if needle in line]
    if not matches:
        return None
    index = matches[0]
    start = 0
    for candidate in range(index, -1, -1):
        if re.match(r"^  [A-Za-z0-9_-]+:\s*$", lines[candidate]):
            start = candidate
            break
    end = len(lines)
    for candidate in range(index + 1, len(lines)):
        if re.match(r"^  [A-Za-z0-9_-]+:\s*$", lines[candidate]):
            end = candidate
            break
    return "\n".join(lines[start:end])


def _section(document: dict, name: str) -> dict:
    """Return a table as a dictionary, so a malformed file fails closed."""
    value = document.get(name)
    return value if isinstance(value, dict) else {}


def _exception_licenses(document: dict) -> list[str]:
    """Return every license identifier allowed through a per-crate exception."""
    licenses = _section(document, "licenses")
    rows = licenses.get("exceptions")
    if not isinstance(rows, list):
        return []
    identifiers: list[str] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        allowed = row.get("allow")
        if isinstance(allowed, list):
            identifiers.extend(item for item in allowed if isinstance(item, str))
    return identifiers


def _check_policy(document: dict, problems: list[str]) -> list[str]:
    licenses = _section(document, "licenses")
    bans = _section(document, "bans")
    sources = _section(document, "sources")
    allowed = licenses.get("allow")
    if not isinstance(allowed, list) or not allowed:
        problems.append(f"{POLICY}: [licenses] allow must be a non-empty array")
        allowed = []
    for license_id in allowed:
        if not isinstance(license_id, str):
            problems.append(f"{POLICY}: every allowed license must be a string")
        elif COPYLEFT.match(license_id):
            problems.append(
                f"{POLICY}: `{license_id}` is copyleft and cannot enter the global"
                " allow list; allow it per crate with a disclosure instead"
            )
        elif license_id not in KNOWN_SPDX:
            problems.append(
                f"{POLICY}: `{license_id}` is not a license this repository accepts;"
                " widen KNOWN_SPDX in dependency-policy-guard.py together with"
                " THIRD-PARTY-NOTICES.md"
            )

    exceptions = licenses.get("exceptions")
    if not isinstance(exceptions, list):
        problems.append(f"{POLICY}: [licenses] exceptions must be an array")
    else:
        seen_crates: set[str] = set()
        for index, row in enumerate(exceptions):
            prefix = f"{POLICY}: licenses.exceptions[{index}]"
            if not isinstance(row, dict):
                problems.append(f"{prefix} must be an object")
                continue
            crate = row.get("crate")
            if not isinstance(crate, str) or not crate:
                problems.append(f"{prefix}: crate must name the dependency")
                continue
            if crate in seen_crates:
                problems.append(f"{prefix}: duplicate crate `{crate}`")
            seen_crates.add(crate)
            exception_allow = row.get("allow")
            if not isinstance(exception_allow, list) or not exception_allow:
                problems.append(f"{prefix}: allow must list the license identifiers")
                continue
            for license_id in exception_allow:
                if not isinstance(license_id, str):
                    problems.append(f"{prefix}: every allowed license must be a string")
                elif license_id in allowed:
                    problems.append(
                        f"{prefix}: `{license_id}` is already globally allowed;"
                        " an exception is only for licenses the graph cannot avoid"
                    )

    if _section(licenses, "private").get("ignore") is not True:
        problems.append(
            f"{POLICY}: [licenses.private] ignore must be true so unpublished"
            " workspace crates stay out of the redistribution check"
        )

    if bans.get("wildcards") != "deny":
        problems.append(f"{POLICY}: [bans] wildcards must be `deny`")
    if bans.get("deny") or bans.get("allow"):
        problems.append(
            f"{POLICY}: [bans] allow/deny crate lists are empty by policy;"
            " a ban needs a named reason and its own guard"
        )

    for key in ("unknown-registry", "unknown-git"):
        if sources.get(key) != "deny":
            problems.append(f"{POLICY}: [sources] {key} must be `deny`")
    if sources.get("allow-registry") != [CRATES_IO_INDEX]:
        problems.append(
            f"{POLICY}: [sources] allow-registry must be exactly `{CRATES_IO_INDEX}`"
        )
    if sources.get("allow-git") not in ([], None):
        problems.append(f"{POLICY}: [sources] allow-git must stay empty")
    return allowed


def check(root: Path = REPO_ROOT) -> list[str]:
    policy_path = root / POLICY
    if not policy_path.is_file():
        return [f"{POLICY}: missing; the workspace has no dependency policy"]
    try:
        text = policy_path.read_text(encoding="utf-8")
        document = tomllib.loads(text)
    except (OSError, tomllib.TOMLDecodeError) as error:
        return [f"{POLICY}: cannot parse: {error}"]

    problems: list[str] = []
    allowed = _check_policy(document, problems)
    allowed_set = {item for item in allowed if isinstance(item, str)}

    if "advisories" not in document:
        problems.append(f"{POLICY}: missing [advisories] section")

    notices = root / NOTICES
    exception_ids = set(_exception_licenses(document))
    if not notices.is_file():
        problems.append(f"{NOTICES}: missing; the disclosure source is required")
    else:
        notices_text = notices.read_text(encoding="utf-8")
        license_guard = _load_license_guard()
        for license_id in license_guard.parse_notices(str(notices)):
            spdx = NOTICES_TO_SPDX.get(license_id)
            if spdx is None:
                problems.append(
                    f"{NOTICES}: `{license_id}` has no SPDX mapping in"
                    " dependency-policy-guard.py"
                )
            elif spdx in VENDORED_ONLY:
                continue
            elif spdx not in allowed_set and spdx not in exception_ids:
                problems.append(
                    f"{NOTICES}: `{license_id}` ships with the repository but"
                    f" `{spdx}` is not allowed in {POLICY}"
                )
        for license_id in sorted(exception_ids):
            if license_id not in notices_text:
                problems.append(
                    f"{POLICY}: the exception license `{license_id}` is not disclosed"
                    f" in {NOTICES}; a copyleft or custom license must be published"
                )

    workflow = root / ECOSYSTEM_WORKFLOW
    if not workflow.is_file():
        problems.append(f"{ECOSYSTEM_WORKFLOW}: missing; no CI runs the dependency policy")
    else:
        workflow_text = workflow.read_text(encoding="utf-8")
        job = _workflow_job_containing(workflow_text, DENY_COMMAND)
        if job is None:
            problems.append(
                f"{ECOSYSTEM_WORKFLOW}: no job runs `{DENY_COMMAND}`;"
                " an unenforced policy is a comment"
            )
        else:
            if TAG_JOB_CONDITION not in job:
                problems.append(
                    f"{ECOSYSTEM_WORKFLOW}: the `{DENY_COMMAND}` job must be tag-gated"
                    f" with `{TAG_JOB_CONDITION.strip()}`"
                )
            for check_name in ("advisories", "bans", "licenses", "sources"):
                if check_name not in job:
                    problems.append(
                        f"{ECOSYSTEM_WORKFLOW}: the `{DENY_COMMAND}` job must name"
                        f" the `{check_name}` check"
                    )
    return problems


class DependencyPolicySelfTest(unittest.TestCase):
    def test_repository_policy_is_consistent(self):
        self.assertEqual(check(REPO_ROOT), [])

    def test_copyleft_in_the_cargo_allow_list_is_rejected(self):
        with _scratch(_policy('"MIT", "Apache-2.0", "GPL-3.0"')) as root:
            problems = check(root)
        self.assertTrue(any("copyleft" in problem for problem in problems))

    def test_disclosed_license_missing_from_policy_is_reported(self):
        with _scratch(_policy('"MIT"')) as root:
            problems = check(root)
        self.assertTrue(
            any("`Apache-2.0` is not allowed" in problem for problem in problems)
        )

    def test_ungated_deny_job_is_rejected(self):
        with _scratch(_policy(), gated=False) as root:
            problems = check(root)
        self.assertTrue(any("tag-gated" in problem for problem in problems))

    def test_exception_license_must_be_disclosed(self):
        policy = _policy(
            '"MIT", "Apache-2.0"',
            exceptions='[[licenses.exceptions]]\ncrate = "demo"\n'
            'allow = ["GPL-3.0-or-later"]\n',
        )
        with _scratch(policy) as root:
            problems = check(root)
        self.assertTrue(any("not disclosed" in problem for problem in problems))

    def test_disclosed_copyleft_exception_is_accepted(self):
        policy = _policy(
            '"MIT", "Apache-2.0"',
            exceptions='[[licenses.exceptions]]\ncrate = "demo"\n'
            'allow = ["GPL-3.0-or-later"]\n',
        )
        with _scratch(policy, notices_extra="GPL-3.0-or-later is used.\n") as root:
            problems = check(root)
        self.assertEqual(problems, [])


def _policy(
    allow: str = '"MIT", "Apache-2.0"',
    exceptions: str = "",
) -> str:
    return f"""[advisories]
ignore = []

[licenses]
allow = [{allow}]

[licenses.private]
ignore = true

[bans]
wildcards = "deny"
deny = []

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["{CRATES_IO_INDEX}"]
allow-git = []

{exceptions}"""


NOTICES_FIXTURE = """# Notices
MIT is used. Apache-2.0 is used. GPL-3.0 covers vendored binaries.
"""


def _workflow(gated: bool) -> str:
    condition = TAG_JOB_CONDITION if gated else "    if: github.event_name == 'push'"
    return (
        "on:\n  workflow_dispatch:\n\njobs:\n  dependency-policy:\n"
        f"{condition}\n    runs-on: ubuntu-26.04\n    steps:\n"
        "      - run: cargo deny check advisories bans licenses sources\n"
    )


class _scratch:
    def __init__(
        self,
        policy: str,
        gated: bool = True,
        notices_extra: str = "",
    ):
        self._policy = policy
        self._gated = gated
        self._notices_extra = notices_extra

    def __enter__(self) -> Path:
        import tempfile

        self._temporary = tempfile.TemporaryDirectory()
        root = Path(self._temporary.name)
        (root / POLICY).write_text(self._policy, encoding="utf-8")
        (root / NOTICES).write_text(
            NOTICES_FIXTURE + self._notices_extra, encoding="utf-8"
        )
        workflow = root / ECOSYSTEM_WORKFLOW
        workflow.parent.mkdir(parents=True, exist_ok=True)
        workflow.write_text(_workflow(self._gated), encoding="utf-8")
        return root

    def __exit__(self, *_excuses) -> None:
        self._temporary.cleanup()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    arguments = parser.parse_args()

    if arguments.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(
            DependencyPolicySelfTest
        )
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    print("dependency policy guard: deny.toml matches the disclosed licenses")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
