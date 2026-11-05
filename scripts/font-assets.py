#!/usr/bin/env python3
"""Validate font asset complete originals and their exact licenses; never subset."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
METADATA_PATH = 'crates/infiltrator-shared/assets/fonts/registry/FONT-ASSETS.json'


def check_collection(registry: dict, root: Path) -> int:
    """Check design originals without selecting products or publishing assets."""
    fonts = registry.get('collectionFonts', [])
    local = registry.get('localCandidates', [])
    if not fonts:
        raise SystemExit('设计字体库为空。')
    all_fonts = [registry['font'], *registry.get('additionalFonts', []), *fonts, *local]
    ids = [font['id'] for font in all_fonts]
    if len(set(ids)) != len(ids) or any(not 0 < len(key) <= 64 for key in ids):
        raise SystemExit('字体ID重复或超出长度限制。')
    available_local = []
    for font in [*fonts, *local]:
        source = root / font['path']
        if font in local and not source.is_file():
            continue
        data = source.read_bytes()
        license_data = (root / font['licensePath']).read_bytes()
        if (
            len(data) != font['bytes']
            or hashlib.sha256(data).hexdigest() != font['sha256']
            or data[:4] not in (b'OTTO', b'\x00\x01\x00\x00')
        ):
            raise SystemExit(f'设计字体完整原件身份不符：{font["id"]}')
        if not license_data or hashlib.sha256(license_data).hexdigest() != font['licenseSha256']:
            raise SystemExit(f'设计字体授权原件身份不符：{font["id"]}')
        if font in fonts and (
            font['license'] != 'SIL OFL 1.1' or b'SIL OPEN FONT LICENSE' not in license_data
        ):
            raise SystemExit(f'可再分发设计字体须保留原始OFL：{font["id"]}')
        if font in local:
            for attachment in font.get('licenseAttachments', []):
                if (
                    hashlib.sha256((root / attachment['path']).read_bytes()).hexdigest()
                    != attachment['sha256']
                ):
                    raise SystemExit(f'补充授权原件身份不符：{font["id"]}')
            available_local.append(font)
    print(
        f'{len(fonts)} collection originals verified; '
        f'{len(available_local)}/{len(local)} local commercial candidates verified; '
        f'all identities and licenses match registry.'
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument(
        '--check-collection',
        action='store_true',
        help='只读核对设计备选库及本机已有的商业候选原件',
    )
    parser.add_argument(
        '--root', type=Path, default=ROOT, help='检查的数据根目录'
    )
    args = parser.parse_args()
    root = args.root.resolve()
    registry_file = root / METADATA_PATH
    if not registry_file.is_file():
        raise SystemExit(f'找不到字体资产注册表：{registry_file}')
    registry = json.loads(registry_file.read_text(encoding='utf-8'))
    return check_collection(registry, root)


if __name__ == '__main__':
    raise SystemExit(main())
