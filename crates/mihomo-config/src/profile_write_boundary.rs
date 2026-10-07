//! Managers of the same canonical store share one in-process write boundary.
use std::collections::HashMap;
use std::env::current_dir;
use std::fs::canonicalize;
use std::io::{ErrorKind, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tokio::sync;

pub(crate) fn shared_boundary(root: &Path) -> Result<Arc<sync::Mutex<()>>> {
    static STORES: OnceLock<Mutex<HashMap<PathBuf, Weak<sync::Mutex<()>>>>> = OnceLock::new();
    let root = canonical_root(root)?;
    let mut stores = STORES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("profile boundary registry poisoned");
    stores.retain(|_, boundary| boundary.strong_count() > 0);
    if let Some(boundary) = stores.get(&root).and_then(Weak::upgrade) {
        return Ok(boundary);
    }
    let boundary = Arc::new(sync::Mutex::new(()));
    stores.insert(root, Arc::downgrade(&boundary));
    Ok(boundary)
}

fn canonical_root(root: &Path) -> Result<PathBuf> {
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        current_dir()?.join(root)
    };
    let mut ancestor = absolute.as_path();
    let mut suffix = Vec::new();
    loop {
        match canonicalize(ancestor) {
            Ok(mut canonical) => {
                for component in suffix.into_iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let Some(name) = ancestor.file_name() else {
                    return Err(error);
                };
                suffix.push(name.to_os_string());
                ancestor = ancestor.parent().ok_or(error)?;
            }
            Err(error) => return Err(error),
        }
    }
}
