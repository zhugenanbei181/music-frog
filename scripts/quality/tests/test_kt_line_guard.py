import importlib.util
import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "kt_line_guard",
    ROOT / "scripts" / "quality" / "kt-line-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class KtLineGuardTests(unittest.TestCase):
    def test_empty_string(self):
        self.assertEqual(GUARD.non_comment_lines(""), 0)
        self.assertEqual(GUARD.non_comment_lines("   \n\n  "), 0)

    def test_single_line_comments(self):
        self.assertEqual(GUARD.non_comment_lines("// comment\n// another"), 0)
        self.assertEqual(GUARD.non_comment_lines("val x = 1 // comment"), 1)

    def test_nested_block_comments(self):
        code = "/* outer /* inner */ still outer */ val x = 10"
        self.assertEqual(GUARD.non_comment_lines(code), 1)

    def test_strings_preserve_comment_syntax(self):
        code = 'val s = "http://example.com//not-a-comment"\nval y = 2'
        self.assertEqual(GUARD.non_comment_lines(code), 2)
        multiline_string = 'val raw = """\n/* inside string */\n// inside\n"""'
        self.assertEqual(GUARD.non_comment_lines(multiline_string), 4)

    def test_violations_detection(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = pathlib.Path(tmpdir)
            android_dir = tmp / "android" / "test"
            android_dir.mkdir(parents=True)
            kt_file = android_dir / "Test.kt"
            kt_file.write_text("\n".join(f"val v{i} = {i}" for i in range(10)), encoding="utf-8")
            
            found = GUARD.violations(tmp, budget=5)
            self.assertEqual(len(found), 1)
            self.assertEqual(found[0][0], 10)
            self.assertEqual(found[0][1], kt_file)

            self.assertEqual(len(GUARD.violations(tmp, budget=15)), 0)


if __name__ == "__main__":
    unittest.main()
