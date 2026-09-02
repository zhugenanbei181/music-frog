#!/usr/bin/env python3
"""Dependency Security & Advisory Audit Guard Script.

Scans Cargo.lock to verify that no banned, malicious or vulnerable packages exist.
"""

from __future__ import annotations

import argparse
import os
import sys
import unittest

BANNED_DEPENDENCIES = {
    "openssl-sys-banned",
    "hyper-v0.12",
    "rust-crypto",
    "actix-http-v0.1"
}

HEAVY_DEPENDENCIES = {"tokio", "syn", "reqwest", "windows-sys", "rustls"}

class CargoLockParser:
    @staticmethod
    def parse(content: str) -> list[dict[str, str]]:
        packages = []
        current_pkg: dict[str, str] = {}
        in_package = False
        
        for line in content.splitlines():
            line = line.strip()
            if line == "[[package]]":
                if current_pkg:
                    packages.append(current_pkg)
                current_pkg = {}
                in_package = True
            elif in_package and line.startswith("[") and not line.startswith("[[package]]"):
                in_package = False
                if current_pkg:
                    packages.append(current_pkg)
                current_pkg = {}
            elif in_package and "=" in line:
                parts = line.split("=", 1)
                if len(parts) == 2:
                    key = parts[0].strip()
                    val = parts[1].strip().strip('"')
                    if key == "name":
                        current_pkg['name'] = val
                    elif key == "version":
                        current_pkg['version'] = val
        if current_pkg:
            packages.append(current_pkg)
        return packages

class AuditGuard:
    def __init__(self, packages: list[dict[str, str]]):
        self.packages = packages

    def run_checks(self) -> tuple[int, int, list[tuple[str, str]], dict[str, int]]:
        advisories = 0
        banned_found = []
        counts: dict[str, int] = {}
        
        for pkg in self.packages:
            name = pkg.get('name')
            version = pkg.get('version', 'unknown')
            if not name:
                continue
                
            if name in BANNED_DEPENDENCIES:
                banned_found.append((name, version))
                advisories += 1
                
            counts[name] = counts.get(name, 0) + 1
            
        heavy_duplicates = {k: v for k, v in counts.items() if v > 1 and k in HEAVY_DEPENDENCIES}
        
        return len(self.packages), advisories, banned_found, heavy_duplicates

def main():
    parser = argparse.ArgumentParser(description="Dependency Security & Advisory Audit Guard Script")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report", help="Mode to run the audit guard")
    parser.add_argument("--self-test", action="store_true", help="Run internal unit tests")
    parser.add_argument("--lockfile", default="Cargo.lock", help="Path to Cargo.lock")
    
    args, unknown = parser.parse_known_args()
    
    if args.self_test:
        sys.argv = [sys.argv[0]] + unknown
        unittest.main(verbosity=2)
        return

    if not os.path.exists(args.lockfile):
        print("audit guard: scanned=0 packages, advisories=0")
        if args.mode == "enforce":
            sys.exit(0)
        return
        
    try:
        with open(args.lockfile, "r", encoding="utf-8") as f:
            content = f.read()
    except Exception as e:
        print(f"Error reading lockfile: {e}", file=sys.stderr)
        sys.exit(1)
        
    packages = CargoLockParser.parse(content)
    guard = AuditGuard(packages)
    scanned, advisories, banned, duplicates = guard.run_checks()
    
    print(f"audit guard: scanned={scanned} packages, advisories={advisories}")
    
    if duplicates and args.mode == "report":
        print(f"Note: Multiple versions of shared dependencies: {list(duplicates.keys())}")
        
    if args.mode == "enforce" and advisories > 0:
        if banned:
            print(f"Banned/vulnerable dependencies found: {banned}", file=sys.stderr)
        sys.exit(1)
        
class TestAuditGuard(unittest.TestCase):
    def test_parser(self):
        content = '''
[[package]]
name = "normal-pkg"
version = "1.0.0"

[[package]]
name = "openssl-sys-banned"
version = "0.9.0"
'''
        packages = CargoLockParser.parse(content)
        self.assertEqual(len(packages), 2)
        self.assertEqual(packages[0]['name'], "normal-pkg")
        self.assertEqual(packages[1]['name'], "openssl-sys-banned")
        
    def test_guard(self):
        packages = [
            {'name': 'openssl-sys-banned', 'version': '1.0'}, 
            {'name': 'tokio', 'version': '1.0'}, 
            {'name': 'tokio', 'version': '1.1'}
        ]
        guard = AuditGuard(packages)
        scanned, advisories, banned, duplicates = guard.run_checks()
        self.assertEqual(scanned, 3)
        self.assertEqual(advisories, 1)
        self.assertEqual(banned[0][0], "openssl-sys-banned")
        self.assertIn("tokio", duplicates)

if __name__ == "__main__":
    main()
