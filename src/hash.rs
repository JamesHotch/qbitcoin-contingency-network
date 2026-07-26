use core::fmt;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sha3::Sha3_256;

/// Fixed-width consensus hash.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hash32(pub [u8; 32]);

impl Hash32 {
    pub const ZERO: Self = Self([0; 32]);

    #[must_use]
    pub fn sha256d(bytes: &[u8]) -> Self {
        let first = Sha256::digest(bytes);
        let second = Sha256::digest(first);
        Self(second.into())
    }

    #[must_use]
    pub fn tagged_sha3(tag: &str, payload: &[u8]) -> Self {
        let tag_hash = Sha3_256::digest(tag.as_bytes());
        let mut hasher = Sha3_256::new();
        hasher.update(tag_hash);
        hasher.update(tag_hash);
        hasher.update(payload);
        Self(hasher.finalize().into())
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for Hash32 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl fmt::Display for Hash32 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_tags_separate_commitments() {
        let payload = b"same bytes";
        assert_ne!(
            Hash32::tagged_sha3("QBTC/TX/v1", payload),
            Hash32::tagged_sha3("QBTC/CHECKPOINT/v1", payload)
        );
    }
}
