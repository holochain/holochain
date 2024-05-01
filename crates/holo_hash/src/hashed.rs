use crate::HasHash;
use crate::HashableContent;
use crate::HoloHashOf;

#[cfg(feature = "serialization")]
use holochain_serialized_bytes::prelude::*;

#[cfg(feature = "fuzzing")]
use crate::PrimitiveHashType;

/// Represents some piece of content along with its hash representation, so that
/// hashes need not be calculated multiple times.
/// Provides an easy constructor which consumes the content.
// MAYBE: consider making lazy with OnceCell
#[cfg_attr(feature = "serialization", derive(Debug, Serialize, Deserialize))]
pub struct HoloHashed<C: HashableContent> {
    /// The content which is hashed of type C.
    pub content: C,
    /// The hash of the content C.
    pub hash: HoloHashOf<C>,
}

impl<C: HashableContent> HasHash<C::HashType> for HoloHashed<C> {
    fn as_hash(&self) -> &HoloHashOf<C> {
        &self.hash
    }

    fn into_hash(self) -> HoloHashOf<C> {
        self.hash
    }
}

#[cfg(feature = "fuzzing")]
impl<'a, C> arbitrary::Arbitrary<'a> for HoloHashed<C>
where
    C: HashableContent + arbitrary::Arbitrary<'a>,
    C::HashType: PrimitiveHashType,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let hash = HoloHashOf::<C>::arbitrary(u)?;
        let content = C::arbitrary(u)?;
        Ok(Self { content, hash })
    }
}

impl<C> HoloHashed<C>
where
    C: HashableContent,
{
    /// Combine content with its precalculated hash
    pub fn with_pre_hashed(content: C, hash: HoloHashOf<C>) -> Self {
        Self { content, hash }
    }

    // NB: as_hash and into_hash are provided by the HasHash impl

    /// Accessor for content
    pub fn as_content(&self) -> &C {
        &self.content
    }

    /// Mutable accessor for content.
    /// Only useful for heavily mocked/fixturated data in testing.
    /// Guaranteed the hash will no longer match the content if mutated.
    #[cfg(feature = "test_utils")]
    pub fn as_content_mut(&mut self) -> &mut C {
        &mut self.content
    }

    /// Convert to content
    pub fn into_content(self) -> C {
        self.content
    }

    /// Deconstruct as a tuple
    pub fn into_inner(self) -> (C, HoloHashOf<C>) {
        (self.content, self.hash)
    }

    /// Convert to a different content type via From
    #[cfg(feature = "test_utils")]
    pub fn downcast<D>(&self) -> HoloHashed<D>
    where
        C: Clone,
        D: HashableContent<HashType = C::HashType> + From<C>,
    {
        HoloHashed {
            content: self.content.clone().into(),
            hash: self.hash.clone(),
        }
    }
}

impl<C> Clone for HoloHashed<C>
where
    C: HashableContent + Clone,
{
    fn clone(&self) -> Self {
        Self {
            content: self.content.clone(),
            hash: self.hash.clone(),
        }
    }
}

impl<C> std::convert::From<HoloHashed<C>> for (C, HoloHashOf<C>)
where
    C: HashableContent,
{
    fn from(g: HoloHashed<C>) -> (C, HoloHashOf<C>) {
        g.into_inner()
    }
}

impl<C> std::ops::Deref for HoloHashed<C>
where
    C: HashableContent,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.as_content()
    }
}

impl<C> std::convert::AsRef<C> for HoloHashed<C>
where
    C: HashableContent,
{
    fn as_ref(&self) -> &C {
        self.as_content()
    }
}

impl<C> std::borrow::Borrow<C> for HoloHashed<C>
where
    C: HashableContent,
{
    fn borrow(&self) -> &C {
        self.as_content()
    }
}

impl<C> std::cmp::PartialEq for HoloHashed<C>
where
    C: HashableContent,
{
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl<C> std::cmp::Eq for HoloHashed<C> where C: HashableContent {}

impl<C> std::hash::Hash for HoloHashed<C>
where
    C: HashableContent,
{
    fn hash<StdH: std::hash::Hasher>(&self, state: &mut StdH) {
        std::hash::Hash::hash(&self.hash, state)
    }
}

impl<C> std::cmp::PartialOrd for HoloHashed<C>
where
    C: HashableContent + PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.content.partial_cmp(&other.content)
    }
}

impl<C> std::cmp::Ord for HoloHashed<C>
where
    C: HashableContent + Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.content.cmp(&other.content)
    }
}

impl<C: HashableContent> HashableContent for HoloHashed<C> {
    type HashType = C::HashType;

    fn hash_type(&self) -> Self::HashType {
        C::hash_type(self)
    }

    fn hashable_content(&self) -> crate::HashableContentBytes {
        crate::HashableContentBytes::Prehashed39(self.as_hash().get_raw_39().to_vec())
    }
}
