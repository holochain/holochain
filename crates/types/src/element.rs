//! Defines a Element, the basic unit of Holochain data.

use crate::{prelude::*, HeaderHashed};
use futures::future::FutureExt;
use holochain_keystore::KeystoreError;
use holochain_serialized_bytes::prelude::*;
pub use holochain_zome_types::element::*;
use holochain_zome_types::entry::Entry;
use must_future::MustBoxFuture;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
/// Element without the hashes for sending across the network
pub struct WireElement {
    /// The signed header for this element
    signed_header: SignedHeader,
    /// If there is an entry associated with this header it will be here
    maybe_entry: Option<Entry>,
}

/// Extension trait to keep zome types minimal
#[async_trait::async_trait]
pub trait ElementExt {
    /// Validate the signature matches the data
    async fn validate(&self) -> Result<(), KeystoreError>;
}

#[async_trait::async_trait]
impl ElementExt for Element {
    /// Validates a chain element
    async fn validate(&self) -> Result<(), KeystoreError> {
        self.signed_header().validate().await?;

        //TODO: make sure that any cases around entry existence are valid:
        //      SourceChainError::InvalidStructure(HeaderAndEntryMismatch(address)),
        Ok(())
    }
}

/// Extension trait to keep zome types minimal
#[async_trait::async_trait]
pub trait SignedHeaderHashedExt {
    /// Create a hash from data
    fn from_content(signed_header: SignedHeader) -> MustBoxFuture<'static, SignedHeaderHashed>;
    /// Sign some content
    async fn new(
        keystore: &KeystoreSender,
        header: HeaderHashed,
    ) -> Result<SignedHeaderHashed, KeystoreError>;
    /// Validate the data
    async fn validate(&self) -> Result<(), KeystoreError>;
}

#[allow(missing_docs)]
#[async_trait::async_trait]
impl SignedHeaderHashedExt for SignedHeaderHashed {
    fn from_content(signed_header: SignedHeader) -> MustBoxFuture<'static, Self>
    where
        Self: Sized,
    {
        async move {
            let (header, signature) = signed_header.into();
            Self::with_presigned(HeaderHashed::from_content(header).await, signature)
        }
        .boxed()
        .into()
    }
    /// SignedHeader constructor
    async fn new(keystore: &KeystoreSender, header: HeaderHashed) -> Result<Self, KeystoreError> {
        let signature = header.author().sign(keystore, &*header).await?;
        Ok(Self::with_presigned(header, signature))
    }

    /// Validates a signed header
    async fn validate(&self) -> Result<(), KeystoreError> {
        if !self
            .header()
            .author()
            .verify_signature(self.signature(), self.header())
            .await?
        {
            return Err(KeystoreError::InvalidSignature(
                self.signature().clone(),
                format!("header {:?}", self.header_address()),
            ));
        }
        Ok(())
    }
}

impl WireElement {
    /// Convert into a [Element] when receiving from the network
    pub async fn into_element(self) -> Element {
        Element::new(
            SignedHeaderHashed::from_content(self.signed_header).await,
            self.maybe_entry,
        )
    }
    /// Convert from a [Element] when sending to the network
    pub fn from_element(e: Element) -> Self {
        let (signed_header, maybe_entry) = e.into_inner();
        Self {
            signed_header: signed_header.into_inner().0,
            maybe_entry: maybe_entry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SignedHeader, SignedHeaderHashed};
    use crate::fixt::*;
    use ::fixt::prelude::*;
    use holo_hash::{HasHash, HoloHashed};

    #[tokio::test(threaded_scheduler)]
    async fn test_signed_header_roundtrip() {
        let signature = SignatureFixturator::new(Unpredictable).next().unwrap();
        let header = HeaderFixturator::new(Unpredictable).next().unwrap();
        let signed_header = SignedHeader(header, signature);
        let hashed: HoloHashed<SignedHeader> = HoloHashed::from_content(signed_header).await;
        let shh: SignedHeaderHashed = hashed.clone().into();

        assert_eq!(shh.header_address(), hashed.as_hash());

        let round: HoloHashed<SignedHeader> = shh.into();

        assert_eq!(hashed, round);
    }
}
