"""Bind compositor pixels to the native renderer readback of the activated Iced state."""
from pathlib import Path
import argparse
import re

from PIL import Image, ImageChops


def compare_rendered_frame(image, bounds=None):
    image = Path(image)
    rendered = image.parent / "rendered-frame.png"
    if not rendered.is_file():
        raise ValueError("missing native renderer readback")
    with Image.open(image) as observed, Image.open(rendered) as expected:
        if observed.size != expected.size:
            raise ValueError("native renderer/compositor dimensions disagree")
        if bounds is None:
            box = (0, 0, *observed.size)
        else:
            x, y, width, height = bounds
            box = (int(x), int(y), int(x + width), int(y + height))
        difference = ImageChops.difference(observed.convert("RGB").crop(box), expected.convert("RGB").crop(box))
        histogram = difference.histogram()
        channels = sum(histogram)
        # Allow only two 8-bit levels of compositor rounding per channel.
        # A preceding layout or missing panel must fail, not settle by a timer.
        disagreeing = sum(histogram[channel * 256 + value] for channel in range(3) for value in range(3, 256))
        mismatch_fraction = disagreeing / channels if channels else 1.0
        if mismatch_fraction > 0.01:
            raise ValueError(f"compositor still shows another native frame ({mismatch_fraction:.2%} channel mismatch)")
    return rendered, mismatch_fraction


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path)
    parser.add_argument("marker", type=Path)
    args = parser.parse_args()
    match = re.search(r"\bbounds=([^\s]+)", args.marker.read_text())
    bounds = [float(value) for value in match[1].split(",")] if match else None
    try:
        compare_rendered_frame(args.image, bounds)
    except (ValueError, OSError) as error:
        parser.exit(1, f"native frame verification: {error}\n")
