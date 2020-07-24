use crate::{HasHash, HashableContent, HoloHash, HoloHashOf};
use holochain_serialized_bytes::SerializedBytesError;

impl<C> HoloHashed<C>
where
    C: HashableContent,
{
    /// Compute the hash of this content and store it alongside
    pub async fn from_content(content: C) -> Self {
        let hash: HoloHashOf<C> = HoloHash::with_data(&content).await;
        Self { content, hash }
    }

    /// Alias for with_content
    // TODO: deprecate
    // #[deprecated = "alias for `from_content`"]
    pub async fn with_data(content: C) -> Result<Self, SerializedBytesError> {
        Ok(Self::from_content(content).await)
    }
}
