use std::sync::Arc;

use bloomfilter::Bloom;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::bin_types::{KitsuneAgent, KitsuneOpHash};

/// An exclusive range of timestamps, measured in microseconds
type TimeWindow = std::ops::Range<Timestamp>;

pub use bloomfilter;
use kitsune_p2p_types::tx2::tx2_utils::PoolBuf;

/// A bloom filter of Kitsune hash types
#[derive(Debug, derive_more::Deref, derive_more::DerefMut, derive_more::From)]
pub struct BloomFilter(bloomfilter::Bloom<MetaOpKey>);

#[cfg(feature = "fuzzing")]
impl proptest::arbitrary::Arbitrary for BloomFilter {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;

        (1usize.., 1usize.., any::<[u8; 32]>())
            .prop_map(|(size, count, seed)| {
                Self(bloomfilter::Bloom::new_with_seed(size, count, &seed))
            })
            .boxed()
    }
}

const TGT_FP: f64 = 0.01;

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

#[derive(Debug)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct TimedBloomFilter {
    /// The bloom filter for the time window.
    /// If this is none then we have no hashes
    /// for this time window.
    pub bloom: Option<BloomFilter>,
    /// The time window for this bloom filter.
    pub time: TimeWindow,
}

/// An encoded timed bloom filter of missing op hashes.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub enum EncodedTimedBloomFilter {
    /// I have no overlap with your agents
    /// Please don't send any ops.
    NoOverlap,
    /// I have overlap and I have no hashes.
    /// Please send all your ops.
    MissingAllHashes {
        /// The time window that we are missing hashes for.
        time_window: TimeWindow,
    },
    /// I have overlap and I have some hashes.
    /// Please send any missing ops.
    HaveHashes {
        /// The encoded bloom filter.
        bloom: EncodedBloom,
        /// The time window these hashes are for.
        time_window: TimeWindow,
    },
}

impl EncodedTimedBloomFilter {
    /// Get the size in bytes of the bloom filter, if one exists
    pub fn size(&self) -> usize {
        match self {
            Self::HaveHashes { bloom, .. } => bloom.len(),
            _ => 0,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, derive_more::Deref)]
#[serde(transparent)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct EncodedBloom(PoolBuf);

impl EncodedBloom {
    pub fn encode(bloom: &BloomFilter) -> Self {
        Self(encode_bloom_filter(bloom))
    }

    pub fn decode(self) -> BloomFilter {
        decode_bloom_filter(&self.0)
    }
}

pub(crate) fn generate_agent_bloom(agents: Vec<AgentInfoSigned>) -> BloomFilter {
    // Create a new bloom with the correct size.
    let mut bloom = bloomfilter::Bloom::new_for_fp_rate(agents.len(), TGT_FP);

    for info in agents {
        let signed_at_ms = info.signed_at_ms;
        // The key is the agent hash + the signed at.
        let key = MetaOpKey::Agent(info.0.agent.clone(), signed_at_ms);
        bloom.set(&key);
    }

    bloom.into()
}

fn encode_bloom_filter(bloom: &BloomFilter) -> PoolBuf {
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

fn decode_bloom_filter(bloom: &[u8]) -> BloomFilter {
    let bitmap_bits = u64::from_le_bytes(*arrayref::array_ref![bloom, 0, 8]);
    let k_num = u32::from_le_bytes(*arrayref::array_ref![bloom, 8, 4]);
    let k1 = u64::from_le_bytes(*arrayref::array_ref![bloom, 12, 8]);
    let k2 = u64::from_le_bytes(*arrayref::array_ref![bloom, 20, 8]);
    let k3 = u64::from_le_bytes(*arrayref::array_ref![bloom, 28, 8]);
    let k4 = u64::from_le_bytes(*arrayref::array_ref![bloom, 36, 8]);
    let sip_keys = [(k1, k2), (k3, k4)];
    bloomfilter::Bloom::from_existing(&bloom[44..], bitmap_bits, k_num, sip_keys).into()
}
