//! Cold-start pipeline pre-compilation cache, mmap static asset tables, and zero-allocation bootstrap.

use bevy::ecs::resource::Resource;

/// Magic prefix identifying a MusicFrog boot pipeline cache record.
pub const BOOT_CACHE_MAGIC: [u8; 4] = *b"MFRG";
/// Serialization version; mismatched records are rejected, never guessed.
pub const BOOT_CACHE_VERSION: u16 = 1;
/// Fixed encoded size: magic(4) + version(2) + three u64 payloads(24) + checksum(8).
pub const BOOT_CACHE_RECORD_LEN: usize = 38;

/// FNV-1a 64-bit hash used as the record integrity checksum.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Read a little-endian `u64` from a slice that has already been length-checked.
fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let mut buffer = [0u8; 8];
    buffer.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(buffer)
}

/// Decoded payload of a validated boot cache record.
#[derive(Debug, PartialEq)]
struct DecodedBootCache {
    shader_count: u64,
    glyph_count: u64,
    duration_ms: u64,
}

/// Why a boot cache record was rejected. Every variant is a distinct,
/// actionable miss that forces a real rebuild rather than a partial restore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootCacheReject {
    Empty,
    BadMagic,
    UnsupportedVersion,
    Truncated,
    ChecksumMismatch,
}

/// Outcome of a cold-start restore attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootCacheRestore {
    /// A valid record was decoded and applied; no rebuild is required.
    Hit,
    /// The cache was absent or unusable; the caller must rebuild and re-record.
    Miss(BootCacheReject),
}

/// Pre-compiled layout metrics and shader pipeline warm-up cache.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BootPipelineCache {
    pub is_warmed_up: bool,
    pub cached_shader_count: usize,
    pub cached_font_glyphs: usize,
    pub boot_duration_ms: u64,
    /// Number of real rebuilds performed. A restore hit never increments it,
    /// so the counter proves a warm cache avoided recompilation.
    pub rebuild_count: u64,
}

impl BootPipelineCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Warm the cache by a real rebuild, incrementing the rebuild counter.
    pub fn mark_warmed(&mut self, shaders: usize, glyphs: usize, duration_ms: u64) {
        self.is_warmed_up = true;
        self.cached_shader_count = shaders;
        self.cached_font_glyphs = glyphs;
        self.boot_duration_ms = duration_ms;
        self.rebuild_count += 1;
    }

    /// Encode a warmed cache into a portable record. An unwarmed cache has
    /// nothing to restore and yields `None`.
    pub fn record(&self) -> Option<Vec<u8>> {
        if !self.is_warmed_up {
            return None;
        }
        let mut bytes = Vec::with_capacity(BOOT_CACHE_RECORD_LEN);
        bytes.extend_from_slice(&BOOT_CACHE_MAGIC);
        bytes.extend_from_slice(&BOOT_CACHE_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(self.cached_shader_count as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.cached_font_glyphs as u64).to_le_bytes());
        bytes.extend_from_slice(&self.boot_duration_ms.to_le_bytes());
        let checksum = fnv1a64(&bytes);
        bytes.extend_from_slice(&checksum.to_le_bytes());
        Some(bytes)
    }

    /// Restore from a record. A hit applies every field and leaves
    /// `rebuild_count` untouched; a corrupt or absent record leaves the cache
    /// unchanged and reports the typed miss reason.
    pub fn restore(&mut self, record: &[u8]) -> BootCacheRestore {
        match Self::decode(record) {
            Ok(decoded) => {
                self.is_warmed_up = true;
                self.cached_shader_count = decoded.shader_count as usize;
                self.cached_font_glyphs = decoded.glyph_count as usize;
                self.boot_duration_ms = decoded.duration_ms;
                BootCacheRestore::Hit
            }
            Err(reason) => BootCacheRestore::Miss(reason),
        }
    }

    fn decode(record: &[u8]) -> Result<DecodedBootCache, BootCacheReject> {
        if record.is_empty() {
            return Err(BootCacheReject::Empty);
        }
        if record.len() < BOOT_CACHE_RECORD_LEN {
            return Err(BootCacheReject::Truncated);
        }
        if record.get(0..4) != Some(&BOOT_CACHE_MAGIC[..]) {
            return Err(BootCacheReject::BadMagic);
        }
        if u16::from_le_bytes([record[4], record[5]]) != BOOT_CACHE_VERSION {
            return Err(BootCacheReject::UnsupportedVersion);
        }
        if fnv1a64(&record[0..30]) != read_u64(record, 30) {
            return Err(BootCacheReject::ChecksumMismatch);
        }
        Ok(DecodedBootCache {
            shader_count: read_u64(record, 6),
            glyph_count: read_u64(record, 14),
            duration_ms: read_u64(record, 22),
        })
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

    /// Iterate the fixed-size entries of this table in order. Entries are
    /// borrowed directly from the static slice, so iteration performs no copy.
    pub fn iter_entries(&self) -> impl Iterator<Item = &'static [u8]> + '_ {
        (0..self.entry_count()).filter_map(move |index| self.get_entry(index))
    }
}

/// Per-frame heap allocation budget meter enforcing zero-allocation steady-state rendering.
///
/// Implements charter law (docs/bevy-ui/BEVY_UI_FRONTEND.md §8.2):
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

    /// Strict zero-allocation check: both the allocation count and byte total
    /// must be zero, not merely within a slack budget.
    pub fn is_zero_alloc(&self) -> bool {
        self.current_frame_allocs == 0 && self.current_frame_bytes == 0
    }

    /// Assert one frame against the steady-state budget, recording it and
    /// returning a typed verdict. An over-budget frame is never reported clean.
    pub fn assert_frame(&mut self, allocs: usize, bytes: usize) -> BudgetVerdict {
        self.record_frame(allocs, bytes);
        if bytes > self.max_allowed_steady_bytes {
            BudgetVerdict::Violation {
                over_bytes: bytes - self.max_allowed_steady_bytes,
            }
        } else {
            BudgetVerdict::Within
        }
    }
}

/// Typed verdict from asserting one frame against the steady-state budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetVerdict {
    Within,
    Violation { over_bytes: usize },
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

    #[test]
    fn test_boot_cache_record_restore_hit_avoids_rebuild() {
        let mut warm = BootPipelineCache::new();
        assert!(warm.record().is_none(), "an unwarmed cache has no record");
        warm.mark_warmed(24, 512, 40);
        assert_eq!(warm.rebuild_count, 1);

        let record = warm.record().expect("warmed cache records bytes");
        assert_eq!(record.len(), BOOT_CACHE_RECORD_LEN);

        // A cold cache restores from the record without any rebuild.
        let mut cold = BootPipelineCache::new();
        assert_eq!(cold.restore(&record), BootCacheRestore::Hit);
        assert!(cold.is_warmed_up);
        assert_eq!(cold.cached_shader_count, 24);
        assert_eq!(cold.cached_font_glyphs, 512);
        assert_eq!(cold.boot_duration_ms, 40);
        assert_eq!(cold.rebuild_count, 0, "a hit must not rebuild");
    }

    #[test]
    fn test_boot_cache_rejects_corrupt_records() {
        let mut warm = BootPipelineCache::new();
        warm.mark_warmed(4, 8, 12);
        let record = warm.record().expect("record");

        assert_eq!(BootPipelineCache::decode(&[]), Err(BootCacheReject::Empty));
        assert_eq!(
            BootPipelineCache::decode(&record[..10]),
            Err(BootCacheReject::Truncated)
        );

        let mut bad_magic = record.clone();
        bad_magic[0] = b'X';
        assert_eq!(
            BootPipelineCache::decode(&bad_magic),
            Err(BootCacheReject::BadMagic)
        );

        let mut bad_version = record.clone();
        bad_version[4] = 0x7F;
        assert_eq!(
            BootPipelineCache::decode(&bad_version),
            Err(BootCacheReject::UnsupportedVersion)
        );

        let mut bad_checksum = record.clone();
        bad_checksum[6] ^= 0xFF;
        assert_eq!(
            BootPipelineCache::decode(&bad_checksum),
            Err(BootCacheReject::ChecksumMismatch)
        );

        // A rejected record never partially applies onto a live cache.
        let mut target = BootPipelineCache::new();
        assert_eq!(
            target.restore(&bad_checksum),
            BootCacheRestore::Miss(BootCacheReject::ChecksumMismatch)
        );
        assert!(!target.is_warmed_up);
        assert_eq!(target.rebuild_count, 0);
    }

    #[test]
    fn test_zero_alloc_assertion_verdicts() {
        let mut meter = ZeroAllocBudgetMeter::new();
        assert!(meter.is_zero_alloc());
        assert_eq!(meter.assert_frame(0, 0), BudgetVerdict::Within);
        assert!(meter.is_zero_alloc());

        assert_eq!(
            meter.assert_frame(1, 64),
            BudgetVerdict::Violation { over_bytes: 64 }
        );
        assert!(!meter.is_zero_alloc());
        assert!(!meter.is_budget_compliant());
        assert_eq!(meter.violation_count, 1);
    }

    #[test]
    fn test_static_byte_table_iterates_entries() {
        static RECORDS: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let table = StaticByteTable::new(&RECORDS, 4);
        let entries: Vec<&[u8]> = table.iter_entries().collect();
        assert_eq!(entries, vec![&[1, 2, 3, 4][..], &[5, 6, 7, 8][..]]);
    }
}
