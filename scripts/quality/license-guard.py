#!/usr/bin/env python3
"""Third-Party License & Open-Source Compliance Guard Script.

Scans THIRD-PARTY-NOTICES.md and verifies license compliance against whitelist.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import unittest

WHITELISTED_LICENSES = {
    "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause",
    "ISC", "Unlicense", "CC0-1.0", "Zlib", "MPL-2.0",
    "SIL OFL 1.1", "OFL-1.1", "CC-BY-4.0", "GPL-3.0"
}

LICENSE_PATTERN = re.compile(
    r'\b(MIT|Apache-2\.0|Apache 2\.0|BSD-2-Clause|BSD-3-Clause|ISC|Unlicense|CC0-1\.0|Zlib|MPL-2\.0|SIL OFL 1\.1|OFL-1\.1|CC-BY 4\.0|CC-BY-4\.0|GPL-3\.0)\b',
    re.IGNORECASE
)

def normalize_license(lic: str) -> str:
    lic = lic.strip()
    mapping = {
        "apache 2.0": "Apache-2.0",
        "apache-2.0": "Apache-2.0",
        "mit": "MIT",
        "bsd-2-clause": "BSD-2-Clause",
        "bsd-3-clause": "BSD-3-Clause",
        "isc": "ISC",
        "unlicense": "Unlicense",
        "cc0-1.0": "CC0-1.0",
        "zlib": "Zlib",
        "mpl-2.0": "MPL-2.0",
        "sil ofl 1.1": "SIL OFL 1.1",
        "ofl-1.1": "SIL OFL 1.1",
        "cc-by 4.0": "CC-BY-4.0",
        "cc-by-4.0": "CC-BY-4.0",
        "gpl-3.0": "GPL-3.0",
    }
    return mapping.get(lic.lower(), lic)

def is_whitelisted(lic: str) -> bool:
    return normalize_license(lic) in WHITELISTED_LICENSES

def parse_notices(filepath: str) -> list[str]:
    if not os.path.exists(filepath):
        return []
    
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    found = []
    for match in LICENSE_PATTERN.finditer(content):
        norm = normalize_license(match.group(1))
        if norm and norm not in found:
            found.append(norm)
            
    return found

class TestLicenseGuard(unittest.TestCase):
    def test_normalize_license(self):
        self.assertEqual(normalize_license("MIT"), "MIT")
        self.assertEqual(normalize_license("mit"), "MIT")
        self.assertEqual(normalize_license("Apache 2.0"), "Apache-2.0")
        self.assertEqual(normalize_license("SIL OFL 1.1"), "SIL OFL 1.1")
        
    def test_is_whitelisted(self):
        self.assertTrue(is_whitelisted("MIT"))
        self.assertTrue(is_whitelisted("Apache-2.0"))
        self.assertTrue(is_whitelisted("SIL OFL 1.1"))
        self.assertTrue(is_whitelisted("GPL-3.0"))
        self.assertFalse(is_whitelisted("Proprietary-Commercial"))
        
    def test_parse_notices(self):
        test_file = "TEST-NOTICES.md"
        with open(test_file, "w") as f:
            f.write("Package A is under MIT license.\nPackage B is under Apache-2.0.\n")
        try:
            licenses = parse_notices(test_file)
            self.assertIn("MIT", licenses)
            self.assertIn("Apache-2.0", licenses)
        finally:
            if os.path.exists(test_file):
                os.remove(test_file)

def main():
    parser = argparse.ArgumentParser(description="Third-Party License & Open-Source Compliance Guard Script")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report", help="Operation mode")
    parser.add_argument("--self-test", action="store_true", help="Run self-tests")
    
    args, unknown = parser.parse_known_args()
    
    if args.self_test:
        sys.argv = [sys.argv[0]] + unknown
        unittest.main()
        return
    
    licenses_found = parse_notices("THIRD-PARTY-NOTICES.md")
    
    violations = 0
    checked = len(licenses_found)
    
    for lic in licenses_found:
        if not is_whitelisted(lic):
            violations += 1
            if args.mode == "enforce":
                print(f"Violation found: {lic} is not whitelisted.")
    
    print(f"license guard: checked={checked} licenses, violations={violations}")
    
    if args.mode == "enforce" and violations > 0:
        sys.exit(1)

if __name__ == "__main__":
    main()
