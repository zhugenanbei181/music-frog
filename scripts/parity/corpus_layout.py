"""Where the published screenshot corpora live — the one true path.

The corpora are **local evidence**, not product sources: no product code reads
a published pixel, every recapture rewrites most of them, and the whole set was
about 60 MB of PNG that the repository carried forever. They now live under
`.evidence/corpora/<surface>/`, beside the index that names them:

* `*.png` — the pixels. Local acceptance evidence, git-ignored.
* `manifest.tsv` — the index (scenario, skin, viewport, bytes, SHA-256). Small,
  tracked, and the only part CI can judge: it holds a surface to its canonical
  matrix without shipping the pixels.

Everything that publishes, judges or audits a corpus reads this module, so
there is exactly one path to keep true. The capture scripts are shell, so they
carry the same literal and `capture-corpus-integrity-guard.py` refuses the tree
if the two ever drift apart.
"""
from __future__ import annotations

import pathlib

CORPUS_ROOT = pathlib.Path(".evidence/corpora")

SLINT_CORPUS = CORPUS_ROOT / "slint"
ICED_CORPUS = CORPUS_ROOT / "iced"
BEVY_CORPUS = CORPUS_ROOT / "bevy"

SURFACE_CORPORA = {
    "slint": SLINT_CORPUS,
    "iced": ICED_CORPUS,
    "bevy": BEVY_CORPUS,
}

# The manifest is the corpus's index; every corpus directory carries exactly
# one, and it is the tracked half of the corpus.
INDEX_NAME = "manifest.tsv"
