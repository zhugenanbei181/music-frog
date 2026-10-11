//! Shared display formatting for byte counts used by peer product surfaces.

const KILOBYTE: u64 = 1024;
const MEGABYTE: u64 = KILOBYTE * 1024;
const GIGABYTE: u64 = MEGABYTE * 1024;
const TERABYTE: u64 = GIGABYTE * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ByteUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
}

impl ByteUnit {
    fn for_count(bytes: u64) -> Self {
        match bytes {
            0..=1023 => Self::Bytes,
            1024..=1_048_575 => Self::Kilobytes,
            1_048_576..=1_073_741_823 => Self::Megabytes,
            1_073_741_824..=1_099_511_627_775 => Self::Gigabytes,
            _ => Self::Terabytes,
        }
    }

    fn divisor(self) -> u64 {
        match self {
            Self::Bytes => 1,
            Self::Kilobytes => KILOBYTE,
            Self::Megabytes => MEGABYTE,
            Self::Gigabytes => GIGABYTE,
            Self::Terabytes => TERABYTE,
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Bytes => "B",
            Self::Kilobytes => "KB",
            Self::Megabytes => "MB",
            Self::Gigabytes => "GB",
            Self::Terabytes => "TB",
        }
    }
}

/// Format a byte count with the shared binary unit ladder used by Iced and Bevy.
pub fn format_bytes(bytes: u64) -> String {
    let unit = ByteUnit::for_count(bytes);
    if unit == ByteUnit::Bytes {
        format!("{bytes} {}", unit.suffix())
    } else {
        format!(
            "{:.2} {}",
            bytes as f64 / unit.divisor() as f64,
            unit.suffix()
        )
    }
}
