use crate::agent_store::AgentInfoSigned;
use crate::meta_net::*;
use crate::types::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::sync::Arc;

/// A bloom filter of Kitsune hash types
pub type BloomFilter = bloomfilter::Bloom<MetaOpKey>;

#[derive(Clone, Debug)]
pub(crate) enum HowToConnect {
    /// The connection handle and the url that this handle has been connected to.
    /// If the connection handle closes the url can change so we need to track it.
    Con(MetaNetCon, String),
    Url(String),
}

/// The key to use for referencing items in a bloom filter
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MetaOpKey {
    /// data key type
    Op(Arc<KitsuneOpHash>),

    /// agent key type
    Agent(Arc<KitsuneAgent>, u64),
}

/// The actual data added to a bloom filter
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MetaOpData {
    /// data chunk type
    Op(Arc<KitsuneOpHash>, Vec<u8>),

    /// agent chunk type
    Agent(AgentInfoSigned),
}

pub(crate) fn encode_bloom_filter(bloom: &BloomFilter) -> PoolBuf {
    let bitmap: Vec<u8> = bloom.bitmap();
    let bitmap_bits: u64 = bloom.number_of_bits();
    let k_num: u32 = bloom.number_of_hash_functions();
    let sip_keys = bloom.sip_keys();
    let k1: u64 = sip_keys[0].0;
    let k2: u64 = sip_keys[0].1;
    let k3: u64 = sip_keys[1].0;
    let k4: u64 = sip_keys[1].1;

    let size = bitmap.len()
        + 8 // bitmap bits
        + 4 // k_num
        + (8 * 4) // k1-4
        ;

    let mut buf = PoolBuf::new();
    buf.reserve(size);

    buf.extend_from_slice(&bitmap_bits.to_le_bytes());
    buf.extend_from_slice(&k_num.to_le_bytes());
    buf.extend_from_slice(&k1.to_le_bytes());
    buf.extend_from_slice(&k2.to_le_bytes());
    buf.extend_from_slice(&k3.to_le_bytes());
    buf.extend_from_slice(&k4.to_le_bytes());
    buf.extend_from_slice(&bitmap);

    buf
}

pub(crate) fn decode_bloom_filter(bloom: &[u8]) -> BloomFilter {
    let bitmap_bits = u64::from_le_bytes(*arrayref::array_ref![bloom, 0, 8]);
    let k_num = u32::from_le_bytes(*arrayref::array_ref![bloom, 8, 4]);
    let k1 = u64::from_le_bytes(*arrayref::array_ref![bloom, 12, 8]);
    let k2 = u64::from_le_bytes(*arrayref::array_ref![bloom, 20, 8]);
    let k3 = u64::from_le_bytes(*arrayref::array_ref![bloom, 28, 8]);
    let k4 = u64::from_le_bytes(*arrayref::array_ref![bloom, 36, 8]);
    let sip_keys = [(k1, k2), (k3, k4)];
    bloomfilter::Bloom::from_existing(&bloom[44..], bitmap_bits, k_num, sip_keys)
}
