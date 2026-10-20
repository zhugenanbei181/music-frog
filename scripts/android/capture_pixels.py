"""Screenshot digests and the interaction verdict for the Android capture flow.

The verdict itself is not re-implemented here: the desktop parity tooling owns
the "did this interaction really change the rendered surface" rule, and this
module reuses that single source of truth so an Android receipt and a desktop
receipt mean the same thing.
"""

from __future__ import annotations

import hashlib
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts" / "parity"))

import interaction_delta  # noqa: E402  (the shared verdict lives in scripts/parity)


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with pathlib.Path(path).open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def png_dimensions(path: pathlib.Path) -> tuple[int, int]:
    from PIL import Image

    with Image.open(path) as image:
        return image.width, image.height


def compare_frames(
    before: pathlib.Path,
    after: pathlib.Path,
    replicate: pathlib.Path | None = None,
    bounds: tuple[float, float, float, float] | None = None,
) -> dict[str, object]:
    """Measure the change between the previous state and the new one.

    ``replicate`` is a second capture of the *previous* state: the difference
    between the two baselines is the noise floor of the surface, and the
    interaction has to beat it.
    """
    noise = 0
    if replicate is not None:
        noise = interaction_delta.noise_floor(before, replicate, bounds)
    verdict = interaction_delta.corroborate(after, before, noise, bounds)
    return {
        "changed_pixels": int(verdict["changed_pixels"]),
        "compared_pixels": int(verdict["compared_pixels"]),
        "noise_pixels": int(verdict["noise_pixels"]),
        "required_pixels": int(verdict["required_pixels"]),
        "corroborated": bool(verdict["corroborated"]),
    }
