//! WTF-8 to UTF-16 conversion for Windows wide-string FFI.
//!
//! The neutral layer may not import Windows-specific standard-library
//! modules, so this module decodes the `OsStr::as_encoded_bytes`
//! representation (WTF-8 on Windows) directly. For every path Windows can hold it produces exactly the same code
//! units as `OsStrExt::encode_wide`, including the unpaired surrogates that
//! WTF-8 permits.

use std::path::Path;

const REPLACEMENT: u16 = 0xFFFD;

/// Encode one WTF-8 byte string as UTF-16 code units, without terminator.
pub fn encode_wide(bytes: &[u8]) -> Vec<u16> {
    let mut units = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let lead = bytes[index];
        let (width, minimum) = match lead {
            0x00..=0x7F => {
                units.push(u16::from(lead));
                index += 1;
                continue;
            }
            0xC2..=0xDF => (2, 0x80),
            0xE0..=0xEF => (3, 0x800),
            0xF0..=0xF4 => (4, 0x1_0000),
            _ => {
                units.push(REPLACEMENT);
                index += 1;
                continue;
            }
        };
        let end = index + width;
        if end > bytes.len() || bytes[index + 1..end].iter().any(|byte| byte & 0xC0 != 0x80) {
            units.push(REPLACEMENT);
            index += 1;
            continue;
        }
        let mask = match width {
            2 => 0x1F,
            3 => 0x0F,
            _ => 0x07,
        };
        let mut scalar = u32::from(lead & mask);
        for byte in &bytes[index + 1..end] {
            scalar = (scalar << 6) | u32::from(byte & 0x3F);
        }
        index = end;
        if scalar < minimum || scalar > 0x10_FFFF {
            units.push(REPLACEMENT);
            continue;
        }
        match scalar {
            0x1_0000..=0x10_FFFF => {
                let offset = scalar - 0x1_0000;
                units.push(0xD800 | (offset >> 10) as u16);
                units.push(0xDC00 | (offset & 0x3FF) as u16);
            }
            // WTF-8 keeps unpaired surrogates as single code units.
            scalar => units.push(scalar as u16),
        }
    }
    units
}

/// NUL-terminated wide representation of one path.
pub fn path(path: &Path) -> Vec<u16> {
    let mut units = encode_wide(path.as_os_str().as_encoded_bytes());
    units.push(0);
    units
}

#[cfg(test)]
mod tests {
    use super::{encode_wide, path};
    use std::path::Path;

    fn ascii(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn ascii_and_bmp_match_utf16() {
        assert_eq!(encode_wide(b"state.json"), ascii("state.json"));
        assert_eq!(
            encode_wide("配置 目录".as_bytes()),
            ascii("配置 目录"),
            "BMP text must match encode_wide"
        );
    }

    #[test]
    fn astral_scalars_become_surrogate_pairs() {
        assert_eq!(encode_wide("\u{1F600}".as_bytes()), ascii("\u{1F600}"));
        assert_eq!(encode_wide("\u{1F600}".as_bytes()), vec![0xD83D, 0xDE00]);
    }

    #[test]
    fn unpaired_wtf8_surrogates_are_preserved() {
        // U+D800 encoded as WTF-8 is ED A0 80.
        assert_eq!(encode_wide(&[0xED, 0xA0, 0x80]), vec![0xD800]);
        assert_eq!(
            encode_wide(&[0x61, 0xED, 0xB0, 0x80, 0x62]),
            vec![0x61, 0xDC00, 0x62]
        );
    }

    #[test]
    fn malformed_input_fails_closed_with_replacement_characters() {
        assert_eq!(encode_wide(&[0xFF]), vec![0xFFFD]);
        assert_eq!(encode_wide(&[0xE4, 0xBD]), vec![0xFFFD, 0xFFFD]);
        assert_eq!(encode_wide(&[0xC0, 0x80]), vec![0xFFFD, 0xFFFD]);
    }

    #[test]
    fn paths_are_nul_terminated() {
        let wide = path(Path::new("data/state.toml"));
        assert_eq!(wide.last(), Some(&0));
        assert_eq!(&wide[..wide.len() - 1], ascii("data/state.toml").as_slice());
    }
}
