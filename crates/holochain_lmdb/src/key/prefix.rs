use super::*;
use holo_hash::AnyDhtHash;
use std::marker::PhantomData;
/// Prefix for integrated database
const INTEGRATED_PREFIX: u8 = 0x0;
/// Prefix for the database awaiting validation ( judgement :) )
const PENDING_PREFIX: u8 = 0x1;
/// Prefix for the database of rejected data (has been judged and found invalid)
const REJECTED_PREFIX: u8 = 0x2;
/// Prefix for authored database
const AUTHORED_PREFIX: u8 = 0x3;

/// Prefix length 1 + hash length 39
const PREFIX_KEY_SIZE: usize = HOLO_HASH_FULL_LEN + 1;

/// A key for hashes but with a prefix for reusing databases
/// This key is optimized for databases where the key is the
/// hash of the data. For other keys use the [PrefixBytesKey].
pub struct PrefixHashKey<P = IntegratedPrefix>
where
    P: PrefixType,
{
    prefix_and_hash: [u8; PREFIX_KEY_SIZE],
    __phantom: PhantomData<P>,
}

/// Key for adding a prefix to a bytes key
#[derive(PartialOrd, Ord, PartialEq, Eq, derive_more::AsRef, Debug, Clone)]
#[as_ref(forward)]
pub struct PrefixBytesKey<P = IntegratedPrefix>
where
    P: PrefixType,
{
    prefix_and_bytes: Vec<u8>,
    #[as_ref(ignore)]
    __phantom: PhantomData<P>,
}

/// Set the prefix type for a prefix
pub trait PrefixType: Ord + Clone + std::fmt::Debug + Send + Sync {
    /// The prefix associated with this prefix
    const PREFIX: u8;
}

#[derive(PartialOrd, Clone, Ord, PartialEq, Eq, Debug)]
/// Prefix key for data that is integrated
pub struct IntegratedPrefix;

#[derive(PartialOrd, Clone, Ord, PartialEq, Eq, Debug)]
/// Prefix key for data that is pending validation
pub struct PendingPrefix;

#[derive(PartialOrd, Clone, Ord, PartialEq, Eq, Debug)]
/// Prefix key for data that has been rejected
pub struct RejectedPrefix;

#[derive(PartialOrd, Clone, Ord, PartialEq, Eq, Debug)]
/// Prefix key for data that has been authored
pub struct AuthoredPrefix;

impl PrefixType for IntegratedPrefix {
    const PREFIX: u8 = INTEGRATED_PREFIX;
}

impl PrefixType for PendingPrefix {
    const PREFIX: u8 = PENDING_PREFIX;
}

impl PrefixType for RejectedPrefix {
    const PREFIX: u8 = REJECTED_PREFIX;
}

impl PrefixType for AuthoredPrefix {
    const PREFIX: u8 = AUTHORED_PREFIX;
}

impl<P: PrefixType> PrefixHashKey<P> {
    /// Create prefix key from a hash
    pub fn new<C>(hash: &HoloHash<C>) -> Self
    where
        C: PrimitiveHashType + Send + Sync,
    {
        let mut key = Self::empty();
        key.fill(P::PREFIX, hash.as_ref());
        key
    }

    fn empty() -> Self {
        Self {
            prefix_and_hash: [0; PREFIX_KEY_SIZE],
            __phantom: PhantomData,
        }
    }

    pub(super) fn fill_from_raw(hash: &[u8]) -> Self {
        if hash.len() != PREFIX_KEY_SIZE {
            panic!(
                "Holochain detected database corruption.\n\nInvalid PrefixHashKey: expected {} bytes but got {}",
                PREFIX_KEY_SIZE,
                hash.len()
            );
        }
        let mut key = Self::empty();

        let data_iter = key.prefix_and_hash.iter_mut();
        let hash_iter = hash.iter();
        Self::fill_data(data_iter, hash_iter);
        key
    }

    fn fill(&mut self, prefix: u8, hash: &[u8]) {
        self.prefix_and_hash[0] = prefix;
        let data_iter = self.prefix_and_hash.iter_mut().skip(1);
        let hash_iter = hash.iter();
        Self::fill_data(data_iter, hash_iter);
    }

    fn fill_data<'a>(
        data_iter: impl Iterator<Item = &'a mut u8>,
        hash_iter: impl Iterator<Item = &'a u8>,
    ) {
        for (data, hash) in data_iter.zip(hash_iter) {
            *data = *hash;
        }
    }

    /// Get the bytes of the hash
    pub fn as_hash_bytes(&self) -> &[u8] {
        let bytes = &self.prefix_and_hash[1..];
        assert_length!(HOLO_HASH_FULL_LEN, bytes);
        bytes
    }
}

impl<P: PrefixType> BufKey for PrefixHashKey<P> {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        Self::fill_from_raw(bytes)
    }
}

impl<P: PrefixType> AsRef<[u8]> for PrefixHashKey<P> {
    fn as_ref(&self) -> &[u8] {
        &self.prefix_and_hash[..]
    }
}

impl<P: PrefixType> PartialEq for PrefixHashKey<P> {
    fn eq(&self, other: &PrefixHashKey<P>) -> bool {
        self.prefix_and_hash[..] == other.prefix_and_hash[..]
    }
}

impl<P: PrefixType> Eq for PrefixHashKey<P> {}

impl<P: PrefixType> PartialOrd for PrefixHashKey<P> {
    fn partial_cmp(&self, other: &PrefixHashKey<P>) -> Option<Ordering> {
        PartialOrd::partial_cmp(&&self.prefix_and_hash[..], &&other.prefix_and_hash[..])
    }
}

impl<P: PrefixType> Ord for PrefixHashKey<P> {
    fn cmp(&self, other: &PrefixHashKey<P>) -> Ordering {
        Ord::cmp(&&self.prefix_and_hash[..], &&other.prefix_and_hash[..])
    }
}

impl<P: PrefixType> BufKey for PrefixBytesKey<P> {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        Self {
            prefix_and_bytes: bytes.to_owned(),
            __phantom: PhantomData,
        }
    }
}

impl<P: PrefixType> PrefixBytesKey<P> {
    /// Create a new prefix bytes key
    pub fn new<I: IntoIterator<Item = u8>>(bytes: I) -> Self {
        PrefixBytesKey {
            prefix_and_bytes: std::iter::once(P::PREFIX).chain(bytes).collect(),
            __phantom: PhantomData,
        }
    }
    /// Get the bytes without the prefix
    pub fn without_prefix(&self) -> &[u8] {
        &self.prefix_and_bytes[1..]
    }
}

impl<T: PrefixType> From<AnyDhtHash> for PrefixBytesKey<T> {
    fn from(h: AnyDhtHash) -> Self {
        Self::new(h.into_iter())
    }
}
