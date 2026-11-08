"""Prove product Bevy guards bind the vendored shared scene executor."""

import ast
import hashlib
import json
import re
import pathlib
import unittest

REPOSITORY = pathlib.Path(__file__).resolve().parents[3]
QUALITY = REPOSITORY / "scripts/quality"
GATE = QUALITY / "bevy_scene_gate.py"
LEDGER = QUALITY / "bevy_scene_ratchet.json"
PRODUCT_GUARDS = (
    QUALITY / "bevy_bsn_guard.py",
    QUALITY / "bevy_rem_guard.py",
)
OWNER_BINDING = re.compile(
    r"mask_noncode\s*=\s*(?:scene_owner|load_shared_scene_owner\(\))\.mask_noncode"
)
FORBIDDEN_DUPLICATES = {
    "_blank",
    "mask_rust",
    "scene_spans",
    "inside_scene",
    "matching_paren",
}
FORBIDDEN_LEXICAL_IMPORT = re.compile(
    r"from\s+(?:bevy_bsn_guard|rust_syntax)\s+import[^\n]*\bmask_noncode\b"
)


class BevyGuardOwnerBindingTests(unittest.TestCase):
    def test_ledger_pins_vendored_owner(self):
        payload = json.loads(LEDGER.read_text(encoding="utf-8"))
        recorded = payload.get("gate", {}).get("sha256")
        actual = hashlib.sha256(GATE.read_bytes()).hexdigest()
        self.assertEqual(recorded, actual)

    def test_product_guards_bind_owner_without_local_lexical_duplicates(self):
        for path in PRODUCT_GUARDS:
            source = path.read_text(encoding="utf-8")
            tree = ast.parse(source, filename=str(path))
            local_definitions = {
                node.name
                for node in tree.body
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            }
            duplicates = sorted(local_definitions & FORBIDDEN_DUPLICATES)
            self.assertEqual(duplicates, [], f"{path.name}: {duplicates}")
            self.assertIn("load_shared_scene_owner", source)
            self.assertIn("bevy_scene_gate.py", source)
            self.assertRegex(source, OWNER_BINDING)
            self.assertIsNone(FORBIDDEN_LEXICAL_IMPORT.search(source))


if __name__ == "__main__":
    unittest.main()
