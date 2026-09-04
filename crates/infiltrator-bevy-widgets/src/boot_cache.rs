//! Cold-start pipeline pre-compilation cache, mmap static asset tables, and zero-allocation bootstrap.

use bevy::ecs::resource::Resource;

/// Pre-compiled layout metrics and shader pipeline warm-up cache.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BootPipelineCache {
    pub is_warmed_up: bool,
    pub cached_shader_count: usize,
    pub cached_font_glyphs: usize,
    pub boot_duration_ms: u64,
}

impl BootPipelineCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_warmed(&mut self, shaders: usize, glyphs: usize, duration_ms: u64) {
        self.is_warmed_up = true;
        self.cached_shader_count = shaders;
        self.cached_font_glyphs = glyphs;
        self.boot_duration_ms = duration_ms;
    }
}

/// Static read-only binary slice table for fast zero-copy memory access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticByteTable {
    pub data: &'static [u8],
    pub entry_size: usize,
}

impl StaticByteTable {
    pub const fn new(data: &'static [u8], entry_size: usize) -> Self {
        Self { data, entry_size }
    }

    pub fn entry_count(&self) -> usize {
        self.data.len().checked_div(self.entry_size).unwrap_or(0)
    }

    pub fn get_entry(&self, index: usize) -> Option<&'static [u8]> {
        let start = index * self.entry_size;
        let end = start + self.entry_size;
        if end <= self.data.len() {
            Some(&self.data[start..end])
        } else {
            None
        }
    }
}

/// Per-frame heap allocation budget meter enforcing zero-allocation steady-state rendering.
///
/// Implements charter law (docs/BEVY_UI_FRONTEND.md §8.2):
/// Once scenes are spawned and pipelines are warm, per-frame restamping must cost 0 heap allocations.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ZeroAllocBudgetMeter {
    pub current_frame_allocs: usize,
    pub current_frame_bytes: usize,
    pub max_allowed_steady_bytes: usize,
    pub violation_count: usize,
}

impl ZeroAllocBudgetMeter {
    pub fn new() -> Self {
        Self {
            current_frame_allocs: 0,
            current_frame_bytes: 0,
            max_allowed_steady_bytes: 0, // strict 0-byte steady-state policy
            violation_count: 0,
        }
    }

    /// Record heap allocations occurred in the current frame.
    pub fn record_frame(&mut self, allocs: usize, bytes: usize) {
        self.current_frame_allocs = allocs;
        self.current_frame_bytes = bytes;
        if bytes > self.max_allowed_steady_bytes {
            self.violation_count += 1;
        }
    }

    /// Whether current frame adhered to zero-allocation budget.
    pub fn is_budget_compliant(&self) -> bool {
        self.current_frame_bytes <= self.max_allowed_steady_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_alloc_budget_meter() {
        let mut meter = ZeroAllocBudgetMeter::new();
        assert!(meter.is_budget_compliant());
        assert_eq!(meter.violation_count, 0);

        // Steady frame: 0 bytes
        meter.record_frame(0, 0);
        assert!(meter.is_budget_compliant());
        assert_eq!(meter.violation_count, 0);

        // Frame with heap churn: 1024 bytes -> violation
        meter.record_frame(2, 1024);
        assert!(!meter.is_budget_compliant());
        assert_eq!(meter.violation_count, 1);
    }
}
