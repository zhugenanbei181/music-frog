"""Stale compositor layouts fail even when page pixels and activation markers exist."""
import tempfile
import unittest
from pathlib import Path

from PIL import Image, ImageDraw
from rendered_frame import compare_rendered_frame


class RenderedFrameTests(unittest.TestCase):
    def test_current_frame_matches_but_an_old_panel_missing_readback_or_wrong_size_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / 'image.png'
            with self.assertRaisesRegex(ValueError, 'missing native'):
                compare_rendered_frame(image)
            native = Image.new('RGB', (720,480), '#333333')
            ImageDraw.Draw(native).rectangle((100,100,300,180), fill='#ffffff')
            native.save(image.parent / 'rendered-frame.png')
            native.save(image)
            self.assertEqual(compare_rendered_frame(image, [100,100,200,80])[1], 0.0)
            Image.new('RGB', (720,480), '#333333').save(image)
            with self.assertRaisesRegex(ValueError, 'another native frame'):
                compare_rendered_frame(image, [100,100,200,80])
            native.resize((1180,780)).save(image)
            with self.assertRaisesRegex(ValueError, 'dimensions disagree'):
                compare_rendered_frame(image)

    def test_only_compositor_rounding_is_allowed_and_changes_outside_the_observed_panel_are_irrelevant(self):
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / 'image.png'
            Image.new('RGB', (720,480), (50,50,50)).save(image.parent / 'rendered-frame.png')
            observed = Image.new('RGB', (720,480), (52,52,52))
            ImageDraw.Draw(observed).rectangle((0,0,80,80), fill='#ffffff')
            observed.save(image)
            self.assertEqual(compare_rendered_frame(image, [100,100,200,80])[1], 0.0)
            with self.assertRaisesRegex(ValueError, 'another native frame'):
                compare_rendered_frame(image)


if __name__ == '__main__':
    unittest.main()
