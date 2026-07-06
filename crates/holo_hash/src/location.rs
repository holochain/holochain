//! Location computation for Holo hashes.

use crate::HoloHashError;

/// internal; compute the Holo dht location `u32`.
pub fn holo_dht_location_bytes(data: &[u8]) -> Vec<u8> {
    // Assert the data size is relatively small so we are
    // comfortable executing this synchronously / blocking tokio thread.
    assert_eq!(32, data.len(), "only 32 byte hashes supported");

    let hash = blake2b_128(data);
    let mut out = vec![hash[0], hash[1], hash[2], hash[3]];
    for i in (4..16).step_by(4) {
        out[0] ^= hash[i];
        out[1] ^= hash[i + 1];
        out[2] ^= hash[i + 2];
        out[3] ^= hash[i + 3];
    }
    out
}

/// Arbitrary (within limits) output length blake2b
pub fn blake2b_n(data: &[u8], length: usize) -> Result<Vec<u8>, HoloHashError> {
    // blake2b_simd does an assert on the hash length and we allow happ devs
    // to set this so we have to put a result guarding against the bounds.
    if !(1..=blake2b_simd::OUTBYTES).contains(&length) {
        return Err(HoloHashError::BadHashSize);
    }
    Ok(blake2b_simd::Params::new()
        .hash_length(length)
        .hash(data)
        .as_bytes()
        .to_vec())
}

/// internal compute a 32 byte blake2b hash
pub fn blake2b_256(data: &[u8]) -> Vec<u8> {
    blake2b_n(data, 32).unwrap()
}

/// internal compute a 16 byte blake2b hash
fn blake2b_128(data: &[u8]) -> Vec<u8> {
    blake2b_n(data, 16).unwrap()
}

/// Compute a 512-bit SHA2 hash.
pub fn sha2_512(data: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    let mut hasher = sha2::Sha512::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.to_vec()
}