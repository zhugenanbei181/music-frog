//! Backwards-local path for the Mihomo-owned bundle materializer.

#[cfg(test)]
use mihomo_version::bundled_kernel::{candidate_paths, copy_bundled_binary, kernel_file_name};

#[cfg(test)]
#[path = "bundled_kernel_tests.rs"]
mod tests;
