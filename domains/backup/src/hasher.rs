use sha2::{Digest, Sha256};

/// Compute SHA-256 hash of input bytes
pub fn sha256_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}
