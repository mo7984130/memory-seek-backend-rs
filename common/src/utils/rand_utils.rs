use rand::{rng, Rng};

#[inline]
pub fn generate_random_str(len: usize) -> String {
    let mut key = vec![0u8; len];
    rng().fill_bytes(&mut *key);
    hex::encode(key)
}