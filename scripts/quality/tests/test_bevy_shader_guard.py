"""Enforce shader authoring boundaries without pretending to compile a shader."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(QUALITY))
SPEC = importlib.util.spec_from_file_location("bevy_shader_guard", QUALITY / "bevy_shader_guard.py")
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class ShaderRules(unittest.TestCase):
    def scan_files(self, files):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / GUARD.ROOTS[0]
            source.mkdir(parents=True)
            for name, content in files.items():
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            return [violation.code for violation in GUARD.scan(root)]

    def test_modular_embedded_sources_and_test_fixtures_are_allowed(self):
        self.assertCountEqual(self.scan_files({
            "shaders/math.wesl": "fn distance() -> f32 { return 0.0; }",
            "entry.wesl": "import package::shaders::math::distance;",
            "assets.rs": 'embedded_asset!(app, "entry.wesl"); embedded_asset!(app, "shaders/math.wesl"); let path = "embedded://widgets/entry.wesl";\n#[cfg(test)] mod tests { const S: &str = r#"@fragment fn f() {}"#; }',
        }), [])

    def test_legacy_format_inline_source_and_leaking_handles_are_rejected(self):
        self.assertCountEqual(self.scan_files({
            "legacy.wgsl": "",
            "assets.rs": 'const SHADER: &str = r#"@fragment fn f() {}"#; mem::forget(handle);',
        }), ["BEVY-SHADER-002", "BEVY-SHADER-003", "BEVY-SHADER-001"])

    def test_unregistered_paths_and_missing_modules_are_rejected(self):
        self.assertCountEqual(self.scan_files({
            "assets.rs": 'let path = "embedded://widgets/entry.wesl";',
            "entry.wesl": "import package::missing::distance;",
        }), ["BEVY-SHADER-004", "BEVY-SHADER-005"])

    def test_cyclic_modules_are_rejected(self):
        self.assertCountEqual(self.scan_files({
            "first.wesl": "import package::second::second;",
            "second.wesl": "import package::first::first;",
        }), ["BEVY-SHADER-004"])

    def test_plain_raw_shaders_are_rejected_but_comments_are_ignored(self):
        self.assertCountEqual(self.scan_files({
            "shader.rs": 'const SOURCE: &str = r"@fragment fn f() {}"; // mem::forget(handle);',
        }), ["BEVY-SHADER-002"])

    def test_legacy_shader_construction_cannot_bypass_file_checks(self):
        self.assertCountEqual(self.scan_files({
            "shader.rs": 'let shader = Shader::from_wgsl(source, "dynamic");',
        }), ["BEVY-SHADER-001"])


if __name__ == "__main__":
    unittest.main()
