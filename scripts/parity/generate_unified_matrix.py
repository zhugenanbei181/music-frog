#!/usr/bin/env python3
"""Generates the unified 182-scenario matrix for Iced, Bevy, and Slint surfaces.

Matrix specification:
- 118 business scenarios: 59 features x 2 viewports (standard + compact), default forest skin
- 64 theme inspection scenarios: 16 core pages x 4 themes (dark, light, forest, amoled)
Total: exactly 182 scenarios per surface.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PARITY_RS = ROOT / "crates/infiltrator-contract/src/parity.rs"

def parse_features():
    content = PARITY_RS.read_text(encoding="utf-8")
    pattern = re.compile(
        r'([A-Za-z0-9_]+)\s*=>\s*\(\s*"([^"]+)"\s*,\s*"([^"]*)"\s*,\s*"([^"]*)"\s*,\s*"([^"]*)"'
    )
    matches = pattern.findall(content)
    features = []
    for var, fid, iced, bevy, slint in matches:
        if fid == "shell-keyboard-focus":
            continue
        features.append((fid, iced, bevy, slint))
    return features

CORE_PAGES = [
    ("overview", "overview", "overview", "overview"),
    ("proxies", "proxies", "proxies", "proxies"),
    ("rules", "rules", "rules", "rules"),
    ("settings", "settings", "settings", "settings"),
    ("profiles", "profiles", "profiles", "profiles"),
    ("connections", "connections", "connections", "connections"),
    ("logs", "logs", "logs", "logs"),
    ("runtime", "runtime", "overview", "runtime"),
    ("dns", "dns", "dns", "dns"),
    ("doctor", "doctor", "doctor", "doctor"),
    ("sync", "sync", "sync", "sync"),
    ("app-routing", "app_routing", "app_routing", "app_routing"),
    ("editor", "editor", "profiles", "editor"),
    ("mixin", "mixin", "profiles", "editor"),
    ("filter", "filter", "profiles", "filters"),
    ("about", "settings", "settings", "about"),
]

# Business captures use the eye-care forest skin by default. Theme inspection
# remains the explicit four-theme matrix so every skin still has real pixels.
BUSINESS_SKIN = "forest"
THEMES = ["dark", "light", "forest", "amoled"]

def generate_surface_tsv(surface: str, out_path: Path):
    features = parse_features()
    lines = ["name\tpage\tskin\twindow_size\n"]
    
    # 1. 118 Business scenarios (alphabetical by feature id)
    for fid, iced, bevy, slint in sorted(features, key=lambda x: x[0]):
        if surface == "iced":
            page = "editor" if fid == "sync-restore-confirm" else (iced if iced else "overview")
            std_dim = "1180x780"
        elif surface == "bevy":
            page = bevy if bevy else "overview"
            std_dim = "1180x780"
        elif surface == "slint":
            page = slint if slint else "Home"
            std_dim = "1180x780"
        else:
            raise ValueError(f"Unknown surface: {surface}")

        lines.append(f"{fid}-standard\t{page}\t{BUSINESS_SKIN}\t{std_dim}\n")
        lines.append(f"{fid}-compact\t{page}\t{BUSINESS_SKIN}\t720x480\n")

    # 2. 64 Theme inspection scenarios (16 pages x 4 themes)
    for page_name, iced_p, bevy_p, slint_p in CORE_PAGES:
        for theme in THEMES:
            if surface == "iced":
                p = iced_p
                std_dim = "1180x780"
            elif surface == "bevy":
                p = bevy_p
                std_dim = "1180x780"
            elif surface == "slint":
                p = slint_p
                std_dim = "1180x780"
            lines.append(f"{page_name}-{theme}\t{p}\t{theme}\t{std_dim}\n")

    out_path.write_text("".join(lines), encoding="utf-8")
    scenario_count = len(lines) - 1
    print(f"[{surface}] Wrote {scenario_count} scenarios to {out_path}")
    assert scenario_count == 182, f"Expected 182 scenarios, got {scenario_count}"

if __name__ == "__main__":
    generate_surface_tsv("slint", ROOT / "scripts/capture_slint_scenarios.tsv")
    generate_surface_tsv("iced", ROOT / "scripts/capture_iced_scenarios.tsv")
    generate_surface_tsv("bevy", ROOT / "scripts/capture_bevy_scenarios.tsv")
