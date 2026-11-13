#!/usr/bin/env python3
"""Every Python package a structure-gate leg needs is declared exactly once.

`GATERUN-003` measured the failure this closes: `main-gate.yml` installed only
`Pillow`, five pixel guards import `numpy`, and eleven legs therefore died on
`ModuleNotFoundError` **before** they could judge anything. The gate reported
17 red legs of four unrelated causes, the ledger declared one of them, and
nothing on the developer machine was red - so the workflow carried no signal and
a real regression could hide inside the existing red sea.

The list lives in `scripts/ci-python-requirements.txt`, and the requirement is
derived, not remembered: the guard parses every `gate python3` leg of
`check-structure.sh`, follows the imports of those scripts (and of the repo
modules they import) and asks which of the reached names are neither stdlib nor
repo-local. Two directions bite:

* a leg that needs a package nobody declares - CI goes red on a machine nobody
  can reproduce;
* a declared package no leg imports - a phantom dependency that gets installed
  into every CI tree forever.

Then every workflow that runs the structure gate must install from the file and
may not hand-type package names, because a per-workflow copy is exactly the
second source of truth that let five install points drift away from six import
sites.
"""
from __future__ import annotations

import argparse
import ast
import contextlib
import pathlib
import re
import shlex
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
REQUIREMENTS = ROOT / "scripts/ci-python-requirements.txt"
GATE = ROOT / "scripts/quality/check-structure.sh"
WORKFLOW_DIR = ROOT / ".github/workflows"
SCRIPTS = ROOT / "scripts"

DECLARATION = re.compile(r"^(?P<package>\S+)\s*(?:#\s*imports:\s*(?P<imports>.*))?$")
GATE_RUNS = ("check-structure.sh", "test.sh --guards-only", "test-bevy.sh --guards-only")
INSTALL_FROM_FILE = "-r scripts/ci-python-requirements.txt"


def gate_legs(gate: pathlib.Path) -> list[str]:
    """Every `gate` command line, continuation lines joined."""
    out: list[str] = []
    pending: str | None = None
    for raw in gate.read_text(encoding="utf-8").splitlines():
        line = raw.rstrip()
        if pending is not None:
            if line.endswith("\\"):
                pending += " " + line[:-1].strip()
            else:
                out.append((pending + " " + line).strip())
                pending = None
            continue
        stripped = line.lstrip()
        if not stripped.startswith("gate "):
            continue
        body = stripped[5:].strip()
        if body.endswith("\\"):
            pending = body[:-1].strip()
        else:
            out.append(body)
    return out


def leg_entry_files(leg: str, root: pathlib.Path) -> list[pathlib.Path]:
    """The script, or the discovered tests, a `python3` leg runs."""
    argv = shlex.split(leg)
    if argv[:2] != ["python3", "-m"]:
        return [root / argv[1]] if argv[1].endswith(".py") else []
    if "-s" not in argv:
        return []
    directory = root / argv[argv.index("-s") + 1]
    pattern = argv[argv.index("-p") + 1] if "-p" in argv else "test_*.py"
    return sorted(directory.glob(pattern))


def module_index(scripts: pathlib.Path) -> dict[str, list[pathlib.Path]]:
    """Import-name -> repo files providing it, so following an import is a lookup."""
    index: dict[str, list[pathlib.Path]] = {}
    for path in scripts.rglob("*.py"):
        index.setdefault(path.stem, []).append(path)
    return index


def top_level_imports(path: pathlib.Path) -> list[str]:
    """Every name a leg may need when it runs, wherever the import sits.

    A function-level import (`design-resource-authority.py` imports numpy
    inside its mascot-detail check) or one behind `try/except ImportError` is
    optional at *module load* but not at runtime: the gate calls that function,
    and a CI tree without the package either degrades silently or reports a
    finding as if it had judged. Both are the drift this guard closes, so every
    import counts and the declaration is mandatory.
    """
    try:
        tree = ast.parse(path.read_text(encoding="utf-8"))
    except (SyntaxError, OSError):
        return []
    names: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            names += [alias.name for alias in node.names]
        elif isinstance(node, ast.ImportFrom) and not node.level and node.module:
            names.append(node.module)
    return names


def needed_modules(gate: pathlib.Path, root: pathlib.Path) -> dict[str, set[str]]:
    """Third-party root names the gate's legs import, with the files that do."""
    scripts = root / "scripts"
    index = module_index(scripts)
    local = set(index)
    stdlib = set(sys.stdlib_module_names)
    queue = [
        path
        for leg in gate_legs(gate)
        if leg.startswith("python3")
        for path in leg_entry_files(leg, root)
    ]
    seen: set[pathlib.Path] = set()
    needed: dict[str, set[str]] = {}
    while queue:
        current = queue.pop()
        if current in seen or not current.is_file():
            continue
        seen.add(current)
        for name in top_level_imports(current):
            base = name.split(".")[0]
            if base in stdlib or base == "__future__":
                continue
            if base in local:
                queue.extend(index[base])
                continue
            needed.setdefault(base, set()).add(str(current.relative_to(root)))
    return needed


def declared_requirements(requirements: pathlib.Path) -> dict[str, tuple[str, set[str]]]:
    """package -> (import module names) as the file declares them."""
    declared: dict[str, tuple[str, set[str]]] = {}
    for line in requirements.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        match = DECLARATION.match(line)
        if not match:
            declared[line] = ("", set())
            continue
        modules = {part.strip() for part in (match.group("imports") or "").split(",") if part.strip()}
        declared[match.group("package")] = (match.group("imports") or "", modules)
    return declared


def gate_workflows(workflow_dir: pathlib.Path) -> list[pathlib.Path]:
    """Workflows that run the structure gate, and therefore owe it packages."""
    found = []
    for path in sorted(workflow_dir.glob("*.y*ml")):
        text = path.read_text(encoding="utf-8")
        if any(marker in text for marker in GATE_RUNS):
            found.append(path)
    return found


def findings(
    gate: pathlib.Path = GATE,
    requirements: pathlib.Path = REQUIREMENTS,
    workflow_dir: pathlib.Path = WORKFLOW_DIR,
    root: pathlib.Path = ROOT,
) -> list[str]:
    problems: list[str] = []
    needed = needed_modules(gate, root)
    declared = declared_requirements(requirements)
    declared_modules = {module for _package, (_text, modules) in declared.items() for module in modules}
    for base in sorted(needed):
        if base not in declared_modules:
            sites = ", ".join(sorted(needed[base])[:3])
            problems.append(
                f"{base} is imported by a structure-gate leg ({sites}) but no "
                f"{requirements.name} line declares it, so every CI tree "
                "that runs the gate dies before the leg can judge anything"
            )
    for package, (text, modules) in sorted(declared.items()):
        if not modules:
            problems.append(
                f"{requirements.name}: `{package}` declares no import name; "
                "the guard can only match a package to the module the legs import"
            )
        for module in sorted(modules):
            if module not in needed:
                problems.append(
                    f"{requirements.name}: `{package}` claims import `{module}`, "
                    "which no structure-gate leg imports - a phantom dependency "
                    "installed into every CI tree"
                )
    workflows = gate_workflows(workflow_dir)
    if not workflows:
        problems.append("no workflow runs the structure gate: the list has no consumer")
    for path in workflows:
        text = path.read_text(encoding="utf-8")
        pip_lines = [line.strip() for line in text.splitlines() if "pip install" in line]
        if not pip_lines:
            problems.append(
                f"{path.relative_to(root)}: runs the structure gate but installs "
                f"no Python dependencies from {requirements.name}"
            )
            continue
        for line in pip_lines:
            if INSTALL_FROM_FILE not in line:
                typed = re.sub(r"^.*pip install\s+", "", line)
                problems.append(
                    f"{path.relative_to(root)}: `{line[:90]}` names packages instead "
                    f"of installing {INSTALL_FROM_FILE}; a per-workflow list is the "
                    "second source of truth that let the gate drift red"
                    + (f" ({typed})" if typed else "")
                )
    return problems


def write_fixture(directory: pathlib.Path, packages: dict[str, str]) -> pathlib.Path:
    """A tree whose gate imports exactly `packages` (module -> file content)."""
    gate = directory / "check-structure.sh"
    lines = []
    for module, body in packages.items():
        script = directory / "leg.py"
        script.write_text(f"import {module}  # noqa\n{body}\n", encoding="utf-8")
        lines.append("gate python3 leg.py")
    gate.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return gate


def requirements_text(packages: dict[str, str]) -> str:
    head = "# planted\n"
    return head + "".join(f"{package}  # imports: {module}\n" for module, package in packages.items())


class GuardSelfTest(unittest.TestCase):
    """Each rule above is proved by planting the defect it forbids."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        (self.root / "scripts").mkdir()

    def fixture(self, modules: tuple[str, ...], declared: dict[str, str]) -> tuple:
        packages = {module: "VALUE = 1\n" for module in modules}
        gate = write_fixture(self.root, packages)
        requirements = self.root / "requirements.txt"
        requirements.write_text(requirements_text(declared), encoding="utf-8")
        workflows = self.root / "workflows"
        workflows.mkdir(exist_ok=True)
        (workflows / "gate.yml").write_text(
            "jobs:\n  guards:\n    steps:\n"
            "      - run: python3 -m pip install -r scripts/ci-python-requirements.txt\n"
            "      - run: bash scripts/quality/check-structure.sh\n",
            encoding="utf-8",
        )
        return gate, requirements, workflows

    def check(self, modules: tuple[str, ...], declared: dict[str, str]) -> list[str]:
        gate, requirements, workflows = self.fixture(modules, declared)
        return findings(gate, requirements, workflows, root=self.root)

    def test_the_live_tree_is_clean(self) -> None:
        self.assertEqual(findings(), [])

    def test_a_leg_needing_an_undeclared_package_is_caught(self) -> None:
        problems = self.check(("numpy",), {"Pillow": "PIL"})
        self.assertTrue(any("numpy is imported by a structure-gate leg" in item for item in problems), problems)

    def test_a_declared_package_no_leg_imports_is_caught(self) -> None:
        problems = self.check(("numpy",), {"numpy": "numpy", "scipy": "scipy"})
        self.assertTrue(any("phantom dependency" in item for item in problems), problems)

    def test_an_exact_declaration_passes(self) -> None:
        self.assertEqual(self.check(("numpy",), {"numpy": "numpy"}), [])

    def test_a_package_without_an_import_name_is_caught(self) -> None:
        gate, _requirements, workflows = self.fixture(("numpy",), {"numpy": "numpy"})
        requirements = self.root / "requirements.txt"
        requirements.write_text("# planted\nnumpy\n", encoding="utf-8")
        problems = findings(gate, requirements, workflows, root=self.root)
        self.assertTrue(any("declares no import name" in item for item in problems), problems)

    def test_a_workflow_that_types_packages_instead_of_reading_the_file_is_caught(self) -> None:
        gate, requirements, workflows = self.fixture(("numpy",), {"numpy": "numpy"})
        (workflows / "gate.yml").write_text(
            "jobs:\n  guards:\n    steps:\n"
            "      - run: python3 -m pip install numpy\n"
            "      - run: bash scripts/quality/check-structure.sh\n",
            encoding="utf-8",
        )
        problems = findings(gate, requirements, workflows, root=self.root)
        self.assertTrue(any("second source of truth" in item for item in problems), problems)

    def test_a_gate_workflow_with_no_install_step_at_all_is_caught(self) -> None:
        gate, requirements, workflows = self.fixture(("numpy",), {"numpy": "numpy"})
        (workflows / "gate.yml").write_text(
            "jobs:\n  guards:\n    steps:\n      - run: bash scripts/quality/check-structure.sh\n",
            encoding="utf-8",
        )
        problems = findings(gate, requirements, workflows, root=self.root)
        self.assertTrue(any("installs no Python dependencies" in item for item in problems), problems)

    def test_an_import_behind_try_except_still_counts_as_needed(self) -> None:
        # A guard that degrades without Pillow would silently stop judging in a
        # CI tree, so declaring it is mandatory.
        gate = self.root / "check-structure.sh"
        (self.root / "leg.py").write_text(
            "import os\ntry:\n    import PIL\nexcept ImportError:\n    PIL = None\n",
            encoding="utf-8",
        )
        gate.write_text("gate python3 leg.py\n", encoding="utf-8")
        requirements = self.root / "requirements.txt"
        requirements.write_text(requirements_text({}), encoding="utf-8")
        workflows = self.root / "workflows"
        workflows.mkdir(exist_ok=True)
        (workflows / "gate.yml").write_text(
            "jobs:\n  guards:\n    steps:\n"
            f"      - run: python3 -m pip install {INSTALL_FROM_FILE}\n"
            "      - run: bash scripts/quality/check-structure.sh\n",
            encoding="utf-8",
        )
        problems = findings(gate, requirements, workflows, root=self.root)
        self.assertTrue(any("PIL is imported" in item for item in problems), problems)

    def test_a_nested_repo_import_is_followed(self) -> None:
        # The leg imports a sibling script by name; that script needs numpy.
        (self.root / "scripts" / "helper.py").write_text("import numpy  # noqa\n", encoding="utf-8")
        gate = self.root / "check-structure.sh"
        (self.root / "leg.py").write_text("import helper\n", encoding="utf-8")
        gate.write_text("gate python3 leg.py\n", encoding="utf-8")
        requirements = self.root / "requirements.txt"
        requirements.write_text(requirements_text({}), encoding="utf-8")
        workflows = self.root / "workflows"
        workflows.mkdir(exist_ok=True)
        (workflows / "gate.yml").write_text(
            "jobs:\n  guards:\n    steps:\n"
            f"      - run: python3 -m pip install {INSTALL_FROM_FILE}\n"
            "      - run: bash scripts/quality/check-structure.sh\n",
            encoding="utf-8",
        )
        problems = findings(gate, requirements, workflows, root=self.root)
        self.assertTrue(any("numpy is imported" in item for item in problems), problems)

    def test_a_lazy_import_inside_a_function_still_counts(self) -> None:
        # `design-resource-authority.py` imports numpy inside a function; at
        # import time nothing breaks, at *runtime* the leg certainly does.
        gate = self.root / "check-structure.sh"
        (self.root / "leg.py").write_text(
            "def check():\n    import numpy  # noqa\n    return True\n",
            encoding="utf-8",
        )
        gate.write_text("gate python3 leg.py --mode enforce\n", encoding="utf-8")
        requirements = self.root / "requirements.txt"
        requirements.write_text(requirements_text({}), encoding="utf-8")
        workflows = self.root / "workflows"
        workflows.mkdir(exist_ok=True)
        (workflows / "gate.yml").write_text(
            "jobs:\n  guards:\n    steps:\n"
            f"      - run: python3 -m pip install {INSTALL_FROM_FILE}\n"
            "      - run: bash scripts/quality/check-structure.sh\n",
            encoding="utf-8",
        )
        problems = findings(gate, requirements, workflows, root=self.root)
        self.assertTrue(any("numpy is imported" in item for item in problems), problems)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    args = parser.parse_args()
    if args.self_test:
        with contextlib.redirect_stdout(sys.stderr):
            result = unittest.TextTestRunner(verbosity=1).run(
                unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
            )
        print(f"ci-dependencies guard: self-test {result.testsRun - len(result.failures) - len(result.errors)}/{result.testsRun} passed")
        return 0 if result.wasSuccessful() else 1
    problems = findings()
    for problem in problems:
        print(f"ci-dependencies guard: {problem}", file=sys.stderr)
    needed = needed_modules(GATE, ROOT)
    summary = ", ".join(f"{name}->{sorted(found)[:1]}" for name, found in sorted(needed.items()))
    print(f"ci-dependencies guard: {len(needed)} third-party import root(s) [{summary}], findings={len(problems)}")
    return 1 if problems and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
