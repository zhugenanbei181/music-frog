# Imported Lumen source archive

> 层级：L3

This directory preserves all 718 non-`.git` files from the supplied
`re-Lumen (1).zip` archive. `SOURCE_IMPORT_MANIFEST.json` records each original
archive path, preserved destination, byte length, and SHA-256 digest. Files
that are retained only as historical reference use a `.source` suffix so old
Cargo, Android, and Lumen documentation files cannot enter MusicFrog builds or
the active documentation map by accident.

The imported Rust services, Android Activity/VPN host, original dependency
lockfile, platform scripts, and repository metadata remain available here for
review. MusicFrog's active host and build metadata live in the workspace. The
adapted Slint Material palette is kept separately from its verbatim upstream
source copy. The only archive content excluded is its `.git/` directory; the
archive hash in the manifest still identifies the exact supplied ZIP.

The active third-surface contract is documented in [Slint surface
contract](../docs/SURFACE.md), while Material adaptations are recorded in
[Lumen patches](material-1.1.0/LUMEN-PATCHES.md).
