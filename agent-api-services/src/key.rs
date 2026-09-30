//! Generation and hashing of API keys.
//!
//! A key looks like `sk_` followed by 64 hex characters (32 random bytes). Only
//! its SHA-256 hash is persisted; the plaintext is shown to the caller once.

use rand::RngCore as _;
use sha2::{Digest as _, Sha256};

/// Number of leading characters of the plaintext kept as a display prefix
/// (`"sk_"` plus seven hex characters).
const PREFIX_LEN: usize = 10;

/// A freshly generated key and its derived storage fields.
#[derive(Debug)]
pub(crate) struct GeneratedKey {
    /// The plaintext key, returned to the caller exactly once.
    pub(crate) plaintext: String,
    /// Short display prefix persisted alongside the hash.
    pub(crate) prefix: String,
    /// Hex-encoded SHA-256 hash persisted for lookups.
    pub(crate) hash: String,
}

/// Generates a new cryptographically random API key.
pub(crate) fn generate() -> GeneratedKey {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);

    let plaintext = format!("sk_{}", hex::encode(bytes));
    let prefix = plaintext[..PREFIX_LEN].to_owned();
    let hash = hash_key(&plaintext);

    GeneratedKey {
        plaintext,
        prefix,
        hash,
    }
}

/// Returns the hex-encoded SHA-256 hash of `key`.
///
/// API keys are high-entropy random values, so a fast cryptographic hash is the
/// right tool here — unlike passwords, they do not need a slow KDF.
pub(crate) fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{PREFIX_LEN, generate, hash_key};

    #[test]
    fn generated_keys_have_expected_shape() {
        let key = generate();
        assert!(key.plaintext.starts_with("sk_"));
        // "sk_" (3) + 32 bytes as hex (64) = 67 characters.
        assert_eq!(key.plaintext.len(), 67);
        assert_eq!(key.prefix.len(), PREFIX_LEN);
        assert!(key.plaintext.starts_with(&key.prefix));
        // SHA-256 is 32 bytes => 64 hex characters, and matches hash_key.
        assert_eq!(key.hash.len(), 64);
        assert_eq!(key.hash, hash_key(&key.plaintext));
    }

    #[test]
    fn generated_keys_are_unique() {
        assert_ne!(generate().plaintext, generate().plaintext);
    }

    #[test]
    fn hashing_is_deterministic_and_distinct() {
        assert_eq!(hash_key("sk_test"), hash_key("sk_test"));
        assert_ne!(hash_key("sk_a"), hash_key("sk_b"));
    }
}
