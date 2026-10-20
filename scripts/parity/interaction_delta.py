#!/usr/bin/env python3
"""Prove that an activated interaction really changed the rendered surface.

A surface that mounts the route of a business but never activates the business
still publishes ``activated=true`` plus a region rectangle: the region is the
page container, not the business control.  The only black-box evidence that the
business itself was exercised is the pixel delta between the interaction
capture and the route-only capture of the same page and viewport.

Animated pages differ between two independent runs, so a single route-only
capture is not a noise floor.  Two route-only captures of the same page and
viewport measure that floor; the interaction delta has to beat it.
"""
from __future__ import annotations

from pathlib import Path

# A per-channel 8-bit difference at or below this level is compositor rounding.
DELTA_LEVEL = 12
# The interaction must be this many times louder than the measured noise floor.
NOISE_MARGIN = 4
# Absolute floor: an interaction that changes fewer pixels than this is noise.
MIN_CHANGED_PIXELS = 16


def _crop(image, bounds):
    if bounds is None:
        return image
    x, y, width, height = bounds
    left, top = int(x), int(y)
    right, bottom = int(x + width), int(y + height)
    if left < 0 or top < 0 or right > image.width or bottom > image.height:
        raise ValueError("interaction region is outside the captured viewport")
    return image.crop((left, top, right, bottom))


def changed_pixels(first, second, bounds=None):
    """Count the pixels that differ between two captures, optionally inside bounds.

    Returns ``(changed, compared)``.  Two captures of different sizes are a
    hard error: they cannot prove anything about each other.
    """
    from PIL import Image, ImageChops

    with Image.open(first) as left, Image.open(second) as right:
        if left.size != right.size:
            raise ValueError("captures of different sizes cannot be compared")
        observed = _crop(left.convert("RGB"), bounds)
        baseline = _crop(right.convert("RGB"), bounds)
        if observed.size != baseline.size:
            raise ValueError("captures of different sizes cannot be compared")
        difference = ImageChops.difference(observed, baseline).convert("L")
        mask = difference.point(lambda value: 255 if value > DELTA_LEVEL else 0)
        return mask.histogram()[255], observed.width * observed.height


def noise_floor(baseline, replicate, bounds=None):
    """The changed-pixel count between two route-only captures of one page."""
    return changed_pixels(baseline, replicate, bounds)[0]


def required_pixels(noise):
    """How many pixels an interaction must change to beat the measured noise."""
    return max(int(noise) * NOISE_MARGIN, MIN_CHANGED_PIXELS)


def corroborate(observed, baseline, noise=0, bounds=None):
    """Decide whether an interaction capture is more than the route-only render.

    ``observed`` is the interaction capture, ``baseline`` the route-only capture
    of the same page and viewport and ``noise`` the changed-pixel count between
    two route-only captures.  The verdict is a plain measurement, never a
    guess about the surface's source code.
    """
    changed, compared = changed_pixels(observed, baseline, bounds)
    needed = required_pixels(noise)
    return {"changed_pixels": changed, "compared_pixels": compared, "noise_pixels": int(noise),
            "required_pixels": needed, "corroborated": changed >= needed and compared > 0}


def main():
    import argparse
    import json

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observed", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--replicate", type=Path, help="second route-only capture: the noise floor")
    parser.add_argument("--bounds", help="x,y,width,height of the interaction region")
    args = parser.parse_args()
    bounds = [float(value) for value in args.bounds.split(",")] if args.bounds else None
    noise = noise_floor(args.baseline, args.replicate, bounds) if args.replicate else 0
    print(json.dumps(corroborate(args.observed, args.baseline, noise, bounds), indent=2))


if __name__ == "__main__":
    main()
