use crate::{HoloHash, HoloHashExt};
use holo_hash_core::{HasHash, HashableContent, HoloHashImpl};
use holochain_serialized_bytes::SerializedBytesError;

/// Represents some piece of content along with its hash representation, so that
/// hashes need not be calculated multiple times.
/// Provides an easy constructor which consumes the content.
// TODO: consider making lazy with OnceCell
pub struct HoloHashed<C: HashableContent> {
    content: C,
    hash: HoloHash<C>,
}

impl<C: HashableContent> HasHash<C::HashType> for HoloHashed<C> {
    fn hash(&self) -> &HoloHash<C> {
        &self.hash
    }

    fn into_hash(self) -> HoloHash<C> {
        self.hash
    }
}

impl<C> HoloHashed<C>
where
    C: HashableContent,
{
    pub async fn from_content(content: C) -> Self {
        let hash: HoloHash<C> = HoloHashImpl::with_data(&content).await.into();
        Self { content, hash }
    }

    pub fn with_pre_hashed(content: C, hash: HoloHash<C>) -> Self {
        Self { content, hash }
    }

    pub fn content(&self) -> &C {
        &self.content
    }

    pub fn into_content(self) -> C {
        self.content
    }

    pub fn into_inner(self) -> (C, HoloHash<C>) {
        (self.content, self.hash)
    }

    // TODO: deprecate
    // #[deprecated = "alias for `from_content`"]
    pub async fn with_data(content: C) -> Result<Self, SerializedBytesError> {
        Ok(Self::from_content(content).await)
    }

    // TODO: deprecate
    // #[deprecated = "alias for `HasHash::hash`"]
    pub fn as_hash(&self) -> &HoloHash<C> {
        &self.hash
    }

    // TODO: deprecate
    // #[deprecated = "alias for `content`"]
    pub fn as_content(&self) -> &C {
        &self.content
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

impl<C> std::fmt::Debug for HoloHashed<C>
where
    C: HashableContent + std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("HoloHashed({:?})", self.content))?;
        Ok(())
    }
}

impl<C> std::convert::From<HoloHashed<C>> for (C, HoloHash<C>)
where
    C: HashableContent,
{
    fn from(g: HoloHashed<C>) -> (C, HoloHash<C>) {
        g.into_inner()
    }
}

impl<C> std::ops::Deref for HoloHashed<C>
where
    C: HashableContent,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.content()
    }
}

impl<C> std::convert::AsRef<C> for HoloHashed<C>
where
    C: HashableContent,
{
    fn as_ref(&self) -> &C {
        self.content()
    }
}

impl<C> std::borrow::Borrow<C> for HoloHashed<C>
where
    C: HashableContent,
{
    fn borrow(&self) -> &C {
        self.content()
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
