"""Unit tests for the neutral crates.io release guard.

The guard owns three contracts: the release topology (every crate of the
chain present, publishable, licensed and version-aligned), the failure
classification of `cargo publish --dry-run` (publish-order gate versus a real
packaging failure) and the refusal to publish without a crates.io API token.
"""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "neutral_release_guard",
    QUALITY / "neutral-release-guard.py",
)
assert SPEC is not None and SPEC.loader is not None
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)

# The chain edges of the real workspace, reduced to the neutral crates.
DEPENDENCIES = {
    "lurker-core-contract": (),
    "lurker-core-ports": ("lurker-core-contract",),
    "lurker-engine-runtime": ("lurker-core-contract", "lurker-core-ports"),
    "lurker-profile-store": ("lurker-core-contract", "lurker-core-ports"),
    "lurker-ui-presentation": ("lurker-core-contract",),
    "mihomo-driver": ("lurker-core-contract", "lurker-core-ports"),
    "singbox-driver": ("lurker-core-contract", "lurker-core-ports"),
}


def write_workspace(
    root: Path,
    *,
    version: str = "0.50.0",
    crates=None,
    dependencies=None,
    license_expression: str = GUARD.NEUTRAL_LICENSE,
    publish_false: bool = False,
    dependency_version: str | None = None,
) -> Path:
    """Build a synthetic workspace that mirrors the neutral release chain."""
    crates = list(GUARD.CANONICAL_ORDER) if crates is None else list(crates)
    dependencies = DEPENDENCIES if dependencies is None else dependencies
    root.mkdir(parents=True, exist_ok=True)
    (root / "Cargo.toml").write_text(
        '[workspace]\nmembers = []\n[workspace.package]\nversion = "'
        + version
        + '"\n',
        encoding="utf-8",
    )
    for name in crates:
        crate = root / "crates" / name
        crate.mkdir(parents=True, exist_ok=True)
        (crate / "src").mkdir(exist_ok=True)
        (crate / "src/lib.rs").write_text("// synthetic\n", encoding="utf-8")
        manifest = [
            "[package]",
            f'name = "{name}"',
            "version.workspace = true",
            'edition = "2024"',
            f'description = "Synthetic {name}."',
            f'license = "{license_expression}"',
            'repository = "https://github.com/zhugenanbei181/music-frog"',
        ]
        if publish_false:
            manifest.append("publish = false")
        edges = dependencies.get(name, ())
        if edges:
            manifest.append("")
            manifest.append("[dependencies]")
            for edge in edges:
                requirement = dependency_version or version
                manifest.append(f'{edge} = {{ version = "{requirement}", path = "../{edge}" }}')
        (crate / "Cargo.toml").write_text("\n".join(manifest) + "\n", encoding="utf-8")
    return root



def workspace_version() -> str:
    """The version every neutral crate publishes, read from the workspace."""
    import tomllib

    manifest = ROOT / "Cargo.toml"
    with manifest.open("rb") as handle:
        data = tomllib.load(handle)
    return str(data["workspace"]["package"]["version"])

class NeutralReleaseTopologyTests(unittest.TestCase):
    def test_repository_chain_is_publishable(self) -> None:
        problems, rows = GUARD.topology_problems(ROOT)
        self.assertEqual(problems, [])
        self.assertEqual([row["crate"] for row in rows], list(GUARD.CANONICAL_ORDER))
        self.assertEqual({row["status"] for row in rows}, {GUARD.STATUS_READY})

    def test_synthetic_chain_is_publishable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            problems, rows = GUARD.topology_problems(root)
        self.assertEqual(problems, [])
        self.assertEqual(len(rows), len(GUARD.CANONICAL_ORDER))

    def test_missing_crate_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            (root / "crates/lurker-core-ports/Cargo.toml").unlink()
            problems, _ = GUARD.topology_problems(root)
        self.assertTrue(
            any("lurker-core-ports" in problem and "missing" in problem for problem in problems),
            problems,
        )

    def test_wrong_publish_order_is_rejected(self) -> None:
        # A crate that depends on a later crate in the chain moves the
        # derived order away from the reviewed one.
        dependencies = dict(DEPENDENCIES)
        dependencies["lurker-ui-presentation"] = ("mihomo-driver",)
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory), dependencies=dependencies)
            problems, _ = GUARD.topology_problems(root)
        self.assertTrue(any("publish order drifted" in problem for problem in problems), problems)

    def test_licence_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory), license_expression="MIT")
            problems, _ = GUARD.topology_problems(root)
        self.assertTrue(any("licence must be" in problem for problem in problems), problems)

    def test_unpublishable_crate_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory), publish_false=True)
            problems, _ = GUARD.topology_problems(root)
        self.assertTrue(any("must stay publishable" in problem for problem in problems), problems)

    def test_dependency_without_version_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            manifest = root / "crates/lurker-core-ports/Cargo.toml"
            manifest.write_text(
                manifest.read_text(encoding="utf-8").replace(
                    'lurker-core-contract = { version = "0.50.0", path = "../lurker-core-contract" }',
                    'lurker-core-contract = { path = "../lurker-core-contract" }',
                ),
                encoding="utf-8",
            )
            problems, _ = GUARD.topology_problems(root)
        self.assertTrue(
            any("explicit `version`" in problem for problem in problems), problems
        )

    def test_dependency_version_must_match_the_release(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory), dependency_version="0.49.0")
            problems, _ = GUARD.topology_problems(root)
        self.assertTrue(
            any("requires version 0.49.0" in problem for problem in problems), problems
        )

    def test_table_lists_every_crate(self) -> None:
        problems, rows = GUARD.topology_problems(ROOT)
        self.assertEqual(problems, [])
        table = GUARD.render_table(rows)
        for name in GUARD.CANONICAL_ORDER:
            self.assertIn(name, table)
        # The version is the workspace's, not a literal in the test: a release
        # bump must not need a test edit.
        self.assertIn(workspace_version(), table)
        markdown = GUARD.render_markdown_table(rows)
        self.assertIn("| lurker-core-contract |", markdown)


class DryRunClassificationTests(unittest.TestCase):
    ORDER_GATE_OUTPUT = (
        "error: failed to select a version for the requirement "
        '`lurker-core-contract = "^0.50.0"`\n'
        "Caused by:\n"
        "  no matching package named `lurker-core-contract` found\n"
        "location searched: crates.io index\n"
    )

    def test_missing_dependency_is_an_order_gate(self) -> None:
        status, reason = GUARD.classify_failure(
            self.ORDER_GATE_OUTPUT, ["lurker-core-contract"], "lurker-core-ports"
        )
        self.assertEqual(status, GUARD.STATUS_ORDER_GATE)
        self.assertIn("lurker-core-contract", reason)

    def test_packaging_failure_is_not_an_order_gate(self) -> None:
        status, _ = GUARD.classify_failure(
            "error: failed to prepare local package for uploading\n",
            ["lurker-core-contract"],
            "lurker-core-ports",
        )
        self.assertEqual(status, GUARD.STATUS_PACKAGING_FAILURE)

    def test_third_party_dependency_failure_is_not_an_order_gate(self) -> None:
        status, _ = GUARD.classify_failure(
            "error: no matching package named `anyhow` found\n",
            ["lurker-core-contract"],
            "lurker-core-ports",
        )
        self.assertEqual(status, GUARD.STATUS_PACKAGING_FAILURE)

    def test_already_published_version_is_reported(self) -> None:
        status, _ = GUARD.classify_failure(
            "error: crate `lurker-core-contract v0.50.0` already exists\n",
            [],
            "lurker-core-contract",
        )
        self.assertEqual(status, GUARD.STATUS_ALREADY_PUBLISHED)

    def test_dry_run_chain_reports_the_order_gate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            _, rows = GUARD.topology_problems(root)

        def runner(crate: str, dry_run: bool, root: Path, timeout: int, token=None):
            if crate == "lurker-core-contract":
                return 0, "packaged"
            return 101, self.ORDER_GATE_OUTPUT

        results = GUARD.dry_run_chain(
            ROOT, rows, runner=runner, probe=lambda name: set()
        )
        self.assertEqual(results[0]["status"], GUARD.STATUS_OK)
        self.assertEqual(results[1]["status"], GUARD.STATUS_ORDER_GATE)
        self.assertEqual(
            [row["status"] for row in results[2:]],
            [GUARD.STATUS_ORDER_GATE] * (len(results) - 2),
        )

    def test_dry_run_chain_reports_a_packaging_failure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            _, rows = GUARD.topology_problems(root)

        def runner(crate: str, dry_run: bool, root: Path, timeout: int, token=None):
            return 101, "error: failed to compile the package\n"

        results = GUARD.dry_run_chain(
            ROOT, rows, runner=runner, probe=lambda name: set()
        )
        self.assertEqual(
            {row["status"] for row in results}, {GUARD.STATUS_PACKAGING_FAILURE}
        )


class PublishContractTests(unittest.TestCase):
    def test_missing_token_is_rejected(self) -> None:
        with self.assertRaises(GUARD.ReleaseError) as caught:
            GUARD.require_token("")
        self.assertIn(GUARD.REGISTRY_TOKEN_ENV, str(caught.exception))

    def test_publish_mode_refuses_to_run_without_a_token(self) -> None:
        environment = dict(os.environ)
        environment.pop(GUARD.REGISTRY_TOKEN_ENV, None)
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            completed = subprocess.run(
                [
                    sys.executable,
                    str(QUALITY / "neutral-release-guard.py"),
                    "--mode",
                    "publish",
                    "--root",
                    str(root),
                ],
                capture_output=True,
                text=True,
                env=environment,
                check=False,
            )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn(GUARD.REGISTRY_TOKEN_ENV, completed.stderr)

    def test_publish_chain_publishes_in_order_and_waits_for_the_index(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            _, rows = GUARD.topology_problems(root)

        published: list[str] = []

        def runner(crate: str, dry_run: bool, root: Path, timeout: int, token=None):
            published.append(crate)
            return 0, "published"

        def probe(name: str) -> set[str]:
            return {"0.50.0"}

        results = GUARD.publish_chain(
            ROOT,
            rows,
            "token",
            runner=runner,
            probe=probe,
            sleeper=lambda seconds: None,
        )
        self.assertEqual(published, list(GUARD.CANONICAL_ORDER))
        self.assertEqual(
            {row["status"] for row in results}, {GUARD.STATUS_PUBLISHED}
        )

    def test_publish_chain_retries_an_order_gate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            _, rows = GUARD.topology_problems(root)

        attempts: dict[str, int] = {}

        def runner(crate: str, dry_run: bool, root: Path, timeout: int, token=None):
            attempts[crate] = attempts.get(crate, 0) + 1
            if crate == "lurker-core-ports" and attempts[crate] == 1:
                return 101, (
                    "error: no matching package named `lurker-core-contract` found\n"
                )
            return 0, "published"

        def probe(name: str) -> set[str]:
            return {"0.50.0"}

        results = GUARD.publish_chain(
            ROOT,
            rows,
            "token",
            runner=runner,
            probe=probe,
            sleeper=lambda seconds: None,
        )
        self.assertEqual(attempts["lurker-core-ports"], 2)
        self.assertEqual(
            {row["status"] for row in results}, {GUARD.STATUS_PUBLISHED}
        )

    def test_publish_chain_passes_the_token_to_cargo(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            _, rows = GUARD.topology_problems(root)

        seen: dict[str, object] = {}

        def runner(crate: str, dry_run: bool, root: Path, timeout: int, token=None):
            seen["token"] = token
            return 0, "published"

        GUARD.publish_chain(
            ROOT,
            rows,
            "secret-token",
            runner=runner,
            probe=lambda name: {"0.50.0"},
            sleeper=lambda seconds: None,
        )
        self.assertEqual(seen["token"], "secret-token")

    def test_the_token_never_reaches_the_command_line(self) -> None:
        for dry_run in (True, False):
            command = GUARD.cargo_publish_command("lurker-core-contract", dry_run)
            self.assertNotIn("--token", command)
            self.assertNotIn("secret", " ".join(command))
        self.assertIn("--dry-run", GUARD.cargo_publish_command("lurker-core-contract", True))
        self.assertNotIn("--dry-run", GUARD.cargo_publish_command("lurker-core-contract", False))

    def test_publish_chain_stops_on_a_packaging_failure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            _, rows = GUARD.topology_problems(root)

        published: list[str] = []

        def runner(crate: str, dry_run: bool, root: Path, timeout: int, token=None):
            published.append(crate)
            return 101, "error: failed to prepare local package for uploading\n"

        results = GUARD.publish_chain(
            ROOT,
            rows,
            "token",
            runner=runner,
            probe=lambda name: set(),
            sleeper=lambda seconds: None,
        )
        self.assertEqual(published, ["lurker-core-contract"])
        self.assertEqual(results[0]["status"], GUARD.STATUS_PACKAGING_FAILURE)
        self.assertEqual(len(results), 1)


class CommandLineTests(unittest.TestCase):
    def test_structure_mode_succeeds_on_a_valid_tree(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            completed = subprocess.run(
                [
                    sys.executable,
                    str(QUALITY / "neutral-release-guard.py"),
                    "--mode",
                    "structure",
                    "--root",
                    str(root),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("lurker-core-contract", completed.stdout)

    def test_structure_mode_fails_on_a_broken_tree(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_workspace(Path(directory))
            (root / "crates/singbox-driver/Cargo.toml").unlink()
            completed = subprocess.run(
                [
                    sys.executable,
                    str(QUALITY / "neutral-release-guard.py"),
                    "--mode",
                    "structure",
                    "--root",
                    str(root),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("singbox-driver", completed.stderr)


if __name__ == "__main__":
    unittest.main()
