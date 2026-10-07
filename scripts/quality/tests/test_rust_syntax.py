"""Lexical regressions that must not hide runtime code from quality boundaries."""
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from rust_syntax import long_paths, mask_comments, mask_noncode


class RustSyntaxTests(unittest.TestCase):
    def test_comment_ending_in_quote_never_starts_a_string(self):
        source = '// label "copy"\nfn action() { crate::page::submit(); }\n'
        masked = mask_noncode(source)
        self.assertIn('fn action() { crate::page::submit(); }', masked)
        self.assertEqual([match.group() for match in long_paths(source)], ['crate::page::submit'])
        self.assertEqual(mask_comments(source).splitlines()[1], source.splitlines()[1])
        self.assertEqual(len(masked), len(source))
        self.assertEqual(masked.count('\n'), source.count('\n'))

    def test_quoted_comment_between_strings_preserves_following_world_entry(self):
        source = 'let before = "caption";\n// next "caption"\nlet after = "copy";\nfn business(world: &mut World) {}'
        masked = mask_noncode(source)
        self.assertIn('fn business(world: &mut World) {}', masked)
        self.assertNotIn('caption', masked)
        self.assertNotIn('copy', masked)

    def test_nested_comments_raw_literals_and_char_literals_still_mask_code_mentions(self):
        source = '''/* nested /* "World" */ comment */
let raw = r##"// World crate::page::submit()"##;
let byte = b'"';
fn runtime() { crate::page::submit(); }
'''
        masked = mask_noncode(source)
        self.assertNotIn('World', masked)
        self.assertEqual([match.group() for match in long_paths(source)], ['crate::page::submit'])
        self.assertIn('r##"// World crate::page::submit()"##', mask_comments(source))


if __name__ == '__main__':
    unittest.main()
