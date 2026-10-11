use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir =
        Path::new(&env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir")).to_path_buf();
    let product_translations =
        manifest_dir.join("../infiltrator-shared/locales/product/translations");
    let bundled_translations =
        PathBuf::from(env::var_os("OUT_DIR").expect("output dir")).join("product-translations");
    for locale in ["en", "zh-CN"] {
        let source = product_translations
            .join(locale)
            .join("LC_MESSAGES/musicfrog.po");
        let target = bundled_translations
            .join(locale)
            .join("LC_MESSAGES/infiltrator-slint.po");
        let parent = target.parent().expect("translation target has a parent");
        fs::create_dir_all(parent).expect("create Slint translation staging directory");
        fs::copy(&source, &target).unwrap_or_else(|error| {
            panic!(
                "stage shared {locale} product catalog for Slint: {source:?} -> {target:?}: {error}"
            )
        });
        println!("cargo:rerun-if-changed={}", source.display());
    }
    for path in [
        "ui/lumen-app.slint",
        "ui/lumen-flags.slint",
        "ui/lumen-ui-scale.slint",
        "lumen-source/material-1.1.0",
        "../infiltrator-shared/assets",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let config = slint_build::CompilerConfiguration::new()
        .with_library_paths(HashMap::from([(
            "material".to_owned(),
            manifest_dir.join("lumen-source/material-1.1.0/material.slint"),
        )]))
        .with_default_translation_context(slint_build::DefaultTranslationContext::None)
        .with_bundled_translations(bundled_translations);

    slint_build::compile_with_config("ui/lumen-app.slint", config)
        .expect("compile imported Lumen Slint surface");
}
