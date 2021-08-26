//! Quickly generate a collection of N 32-byte arrays whose computed DHT locations
//! are evenly distributed across the space of u32 values.
//! Specifically, there is only one hash per interval of size `(2 ^ 32) / N`

use holo_hash::encode::holo_dht_location_bytes;
use holo_hash::*;
use rand::RngCore;

const N: usize = u8::MAX as usize;

fn main() {
    let mut tot = 0;
    let mut hashes: [Option<[u8; 36]>; N] = [None; N];
    let mut hash = [0u8; 36];
    let mut rng = rand::thread_rng();
    while tot < N {
        for i in (0..32).step_by(8) {
            hash[i..i + 8].copy_from_slice(&rng.next_u64().to_le_bytes());
        }
        let loc_bytes = holo_dht_location_bytes(&hash[0..32]);
        hash[32..].copy_from_slice(&loc_bytes);
        let idx = bytes_to_loc(loc_bytes).to_u32() / (u32::MAX / N as u32);

        match &mut hashes[idx as usize] {
            Some(_) => (),
            h @ None => {
                *h = Some(hash);
                tot += 1;
            }
        }
    }
    assert!(hashes.iter().all(|h| h.is_some()));
    hashes.into_iter().map(|h| holo_hash_encode(&h.unwrap()))
}
