use crate::{HashableContent, HoloHash, HoloHashOf, HoloHashed};

impl<C> HoloHashed<C>
where
    C: HashableContent,
{
    /// Compute the hash of this content and store it alongside
    pub async fn from_content(content: C) -> Self {
        let hash: HoloHashOf<C> = HoloHash::with_data(&content).await;
        Self { content, hash }
    }
}
