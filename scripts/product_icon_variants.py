#!/usr/bin/env python3
"""Derive and audit every product icon variant from one canonical artwork.

The shared px ladder, the Android density buckets, and the multi-frame ICO
are all generated from the single `icon.png` registered in
`product-identity.properties`, so every surface binds the same identity.
Contrast against light, dark, and accent backgrounds is audited per variant.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from product_identity import read_properties

ROOT = Path(__file__).resolve().parents[1]
PRODUCT_ASSETS = ROOT / "crates/infiltrator-shared/assets/product"
ACCENT_BACKGROUND = (255, 152, 0)
MIN_CONTRAST = 3.0
MIN_COVERAGE = 0.85


def variant_matrix(identity: dict[str, str]) -> tuple[list[int], list[tuple[str, int]]]:
    try:
        ladder = [int(item) for item in identity["product.icon_variant_px"].split(",")]
        densities: list[tuple[str, int]] = []
        for entry in identity["product.android_icon_density"].split(","):
            density, _, pixels = entry.partition(":")
            densities.append((density.strip(), int(pixels)))
    except (KeyError, ValueError) as error:
        raise ValueError(f"invalid product icon variant matrix: {error}") from error
    if not ladder or any(pixels <= 0 for pixels in ladder):
        raise ValueError("product.icon_variant_px must list positive pixel sizes")
    if not densities or any(pixels <= 0 for _, pixels in densities):
        raise ValueError("product.android_icon_density must list density:px pairs")
    if len({pixels for _, pixels in densities}) != len(densities):
        raise ValueError("product.android_icon_density must not repeat a pixel size")
    return ladder, densities


def relative_luminance(color: tuple[float, float, float]) -> float:
    def channel(value: float) -> float:
        value /= 255.0
        return value / 12.92 if value <= 0.03928 else ((value + 0.055) / 1.055) ** 2.4

    red, green, blue = color
    return 0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue)


def contrast_ratio(one: float, other: float) -> float:
    lighter, darker = max(one, other), min(one, other)
    return (lighter + 0.05) / (darker + 0.05)


def audit_variant(path: Path, expected_pixels: int) -> list[str]:
    problems: list[str] = []
    image = Image.open(path).convert("RGBA")
    if image.size != (expected_pixels, expected_pixels):
        problems.append(f"{path.name}: expected {expected_pixels}px, found {image.size[0]}px")
        return problems
    flattened = image.get_flattened_data() if hasattr(image, "get_flattened_data") else image.getdata()
    opaque = [pixel[:3] for pixel in flattened if pixel[3] >= 128]
    coverage = len(opaque) / (expected_pixels * expected_pixels)
    if coverage < MIN_COVERAGE:
        problems.append(f"{path.name}: opaque coverage {coverage:.2f} below {MIN_COVERAGE}")
        return problems
    mean = tuple(sum(channel[i] for channel in opaque) / len(opaque) for i in range(3))
    content_luminance = relative_luminance(mean)
    for name, background in (
        ("light", (255, 255, 255)),
        ("dark", (0, 0, 0)),
        ("accent", ACCENT_BACKGROUND),
    ):
        ratio = contrast_ratio(content_luminance, relative_luminance(background))
        if ratio < MIN_CONTRAST:
            problems.append(
                f"{path.name}: {name} background contrast {ratio:.2f} below {MIN_CONTRAST}"
            )
    return problems


def generate(ladder: list[int], densities: list[tuple[str, int]]) -> list[str]:
    canonical_path = PRODUCT_ASSETS / "icon.png"
    canonical = Image.open(canonical_path).convert("RGBA")
    problems: list[str] = []
    frames = []
    for pixels in ladder:
        frame = canonical.resize((pixels, pixels), Image.Resampling.LANCZOS)
        target = PRODUCT_ASSETS / f"icon-{pixels}.png"
        frame.save(target)
        frames.append(frame)
        problems.extend(audit_variant(target, pixels))
    # Pillow derives the ICO canvas limit from the base image, so the largest
    # frame must be the base and the smaller frames ride along appended.
    frames.sort(key=lambda frame: frame.size[0], reverse=True)
    frames[0].save(
        PRODUCT_ASSETS / "icon.ico",
        format="ICO",
        append_images=frames[1:],
        sizes=[(pixels, pixels) for pixels in ladder],
    )
    for density, pixels in densities:
        target = PRODUCT_ASSETS / f"icon-android-{density}.png"
        canonical.resize((pixels, pixels), Image.Resampling.LANCZOS).save(target)
        problems.extend(audit_variant(target, pixels))
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    args = parser.parse_args()
    identity = read_properties(ROOT / "product-identity.properties")
    ladder, densities = variant_matrix(identity)
    problems = generate(ladder, densities)
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        return 1
    rendered = ", ".join(str(pixels) for pixels in ladder)
    android = ", ".join(f"{density}:{pixels}" for density, pixels in densities)
    print(f"product icon variants regenerated: px ladder [{rendered}], android [{android}]")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
