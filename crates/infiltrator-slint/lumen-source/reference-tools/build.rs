use std::{collections::HashMap, env, path::Path};

fn main() {
    let manifest_dir = Path::new(&env::var_os("CARGO_MANIFEST_DIR").unwrap()).to_path_buf();
    let config = slint_build::CompilerConfiguration::new()
        .with_library_paths(HashMap::from([(
            "material".to_string(),
            manifest_dir.join("material-1.1.0/material.slint"),
        )]))
        .with_default_translation_context(slint_build::DefaultTranslationContext::None)
        .with_bundled_translations(manifest_dir.join("translations"));

    slint_build::compile_with_config("ui/app.slint", config)
        .expect("failed to compile Lumen Slint UI");
}
