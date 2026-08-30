use std::fmt;
use std::ptr;
use std::sync::atomic::{compiler_fence, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretType {
    ProxyPassword,
    ApiToken,
    PrivateKey,
    SubscriptionUrl,
}

pub struct ProtectedSecret {
    data: Vec<u8>,
    secret_type: SecretType,
}

impl ProtectedSecret {
    /// Creates a new ProtectedSecret from a string and a specific secret type.
    pub fn new(secret: &str, secret_type: SecretType) -> Self {
        Self {
            data: secret.as_bytes().to_vec(),
            secret_type,
        }
    }

    /// Exposes the inner secret as a string reference.
    pub fn expose_secret(&self) -> &str {
        // Safe because the data is initialized from a valid UTF-8 string and never modified.
        unsafe { std::str::from_utf8_unchecked(&self.data) }
    }

    /// Returns the type of the secret.
    pub fn secret_type(&self) -> SecretType {
        self.secret_type
    }

    /// Returns a masked preview of the secret.
    /// Short strings (<= 4 chars) are completely masked.
    /// Longer strings show the first 3 and last 3 characters.
    pub fn masked_preview(&self) -> String {
        let s = self.expose_secret();
        let len = s.chars().count();
        if len <= 4 {
            return "*".repeat(len);
        }
        
        let first = s.chars().take(3).collect::<String>();
        let last = s.chars().skip(len - 3).collect::<String>();
        format!("{}***{}", first, last)
    }
}

impl Drop for ProtectedSecret {
    fn drop(&mut self) {
        if !self.data.is_empty() {
            unsafe {
                // Volatile write to zero out memory securely
                for i in 0..self.data.len() {
                    ptr::write_volatile(self.data.as_mut_ptr().add(i), 0);
                }
            }
            compiler_fence(Ordering::SeqCst);
        }
    }
}

impl fmt::Debug for ProtectedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProtectedSecret([REDACTED])")
    }
}

impl Clone for ProtectedSecret {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            secret_type: self.secret_type,
        }
    }
}

impl PartialEq for ProtectedSecret {
    fn eq(&self, other: &Self) -> bool {
        if self.data.len() != other.data.len() {
            return false;
        }
        let mut result = 0;
        for (a, b) in self.data.iter().zip(other.data.iter()) {
            result |= a ^ b;
        }
        result == 0 && self.secret_type == other.secret_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_encapsulation_and_expose() {
        let secret = ProtectedSecret::new("my_super_secret", SecretType::ProxyPassword);
        assert_eq!(secret.expose_secret(), "my_super_secret");
        assert_eq!(secret.secret_type(), SecretType::ProxyPassword);
    }

    #[test]
    fn test_debug_redaction() {
        let secret = ProtectedSecret::new("hidden_data", SecretType::ApiToken);
        let debug_str = format!("{:?}", secret);
        assert_eq!(debug_str, "ProtectedSecret([REDACTED])");
        assert!(!debug_str.contains("hidden_data"));
    }

    #[test]
    fn test_masked_preview() {
        let short = ProtectedSecret::new("abc", SecretType::PrivateKey);
        assert_eq!(short.masked_preview(), "***");

        let exactly_four = ProtectedSecret::new("1234", SecretType::PrivateKey);
        assert_eq!(exactly_four.masked_preview(), "****");

        let long = ProtectedSecret::new("abcdefghij", SecretType::SubscriptionUrl);
        assert_eq!(long.masked_preview(), "abc***hij");
    }

    #[test]
    fn test_clone_and_eq() {
        let s1 = ProtectedSecret::new("password", SecretType::ProxyPassword);
        let s2 = s1.clone();
        assert_eq!(s1, s2);
        
        let s3 = ProtectedSecret::new("password1", SecretType::ProxyPassword);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_drop() {
        let secret = ProtectedSecret::new("to_be_dropped", SecretType::ApiToken);
        drop(secret); // Ensure it does not panic during zeroization
    }
}
