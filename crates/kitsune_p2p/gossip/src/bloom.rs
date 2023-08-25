use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::bin_types::{KitsuneAgent, KitsuneOpHash};
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::sync::Arc;

type BloomInner = bloomfilter::Bloom<MetaOpKey>;

/// An exclusive range of timestamps, measured in microseconds
pub type TimeWindow = std::ops::Range<Timestamp>;

/// A bloom filter of Kitsune hash types
#[derive(
    Debug,
    Clone,
    derive_more::From,
    derive_more::Deref,
    derive_more::DerefMut,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct BloomFilter(
    #[serde(
        serialize_with = "encode_bloom_filter",
        deserialize_with = "decode_bloom_filter"
    )]
    BloomInner,
);

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

impl PartialEq for BloomFilter {
    fn eq(&self, other: &Self) -> bool {
        self.bit_vec() == other.bit_vec()
            && self.number_of_bits() == other.number_of_bits()
            && self.number_of_hash_functions() == other.number_of_hash_functions()
            && self.sip_keys() == other.sip_keys()
    }
}

impl Eq for BloomFilter {}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum TimedBloomFilter {
    NoOverlap,

    MissingAllHashes {
        window: TimeWindow,
    },

    HaveHashes {
        window: TimeWindow,
        bloom: BloomFilter,
    },
}

#[cfg(feature = "fuzzing")]
impl proptest::arbitrary::Arbitrary for BloomFilter {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;

        (
            // proptest::collection::vec(any::<u8>(), 1..65536),
            any::<Vec<u8>>(),
            // any::<u64>(),
            any::<u32>(),
            any::<[(u64, u64); 2]>(),
        )
            .prop_map(|(bloom, num, keys)| {
                Self(bloomfilter::Bloom::from_existing(
                    &bloom,
                    bloom.len() as u64,
                    num,
                    keys,
                ))
            })
            .boxed()
    }
}

#[cfg(feature = "fuzzing")]
impl<'a> arbitrary::Arbitrary<'a> for BloomFilter {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let size = 1 + u.choose_index(usize::MAX / 8 - 1)?;
        Ok(Self(bloomfilter::Bloom::new_with_seed(
            size,
            1usize.saturating_add(u.arbitrary()?),
            &u.arbitrary()?,
        )))
    }
}

fn encode_bloom_filter<S: serde::Serializer>(
    bloom: &BloomInner,
    ser: S,
) -> Result<S::Ok, S::Error> {
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

    ser.serialize_bytes(&buf)
}

fn decode_bloom_filter<'de, D: serde::Deserializer<'de>>(de: D) -> Result<BloomInner, D::Error> {
    de.deserialize_bytes(BloomBytesVisitor)
}

struct BloomBytesVisitor;

impl<'de> serde::de::Visitor<'de> for BloomBytesVisitor {
    type Value = BloomInner;

    fn visit_bytes<E>(self, bloom: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let bitmap_bits = u64::from_le_bytes(*arrayref::array_ref![bloom, 0, 8]);
        let k_num = u32::from_le_bytes(*arrayref::array_ref![bloom, 8, 4]);
        let k1 = u64::from_le_bytes(*arrayref::array_ref![bloom, 12, 8]);
        let k2 = u64::from_le_bytes(*arrayref::array_ref![bloom, 20, 8]);
        let k3 = u64::from_le_bytes(*arrayref::array_ref![bloom, 28, 8]);
        let k4 = u64::from_le_bytes(*arrayref::array_ref![bloom, 36, 8]);
        let sip_keys = [(k1, k2), (k3, k4)];

        // TODO: check for invalid settings when decoding over the wire
        Ok(bloomfilter::Bloom::from_existing(
            &bloom[44..],
            bitmap_bits,
            k_num,
            sip_keys,
        ))
    }

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("kitsune-encoded bloom filter")
    }
}
