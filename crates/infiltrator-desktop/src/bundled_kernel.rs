//! Backwards-local path for the Mihomo-owned bundle materializer.

pub use mihomo_version::bundled_kernel::{
    bundled_binary_sha256, candidate_paths, copy_bundled_binary, kernel_file_name,
    packaged_candidates,
};

#[cfg(test)]
#[path = "bundled_kernel_tests.rs"]
mod tests;
