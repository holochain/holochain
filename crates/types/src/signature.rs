//! The Signature type is defined here. They are used in ChainHeaders as
//! a way of providing cryptographically verifiable proof of a given agent
//! as having been the author of a given data entry.

use crate::{persistence::cas::content::Address, prelude::*};

/// Provenance is a tuple of initiating agent public key and signature of some item being signed
/// this type is used in headers and in capability requests where the item being signed
/// is implicitly known by context
#[derive(Clone, Debug, Serialize, Default, Deserialize, PartialEq, Hash, Eq, SerializedBytes)]
pub struct Provenance(Address, Signature);

impl Provenance {
    /// Creates a new provenance instance with source typically
    /// being an agent address (public key) and the signature
    /// some signed data using the private key associated with
    /// the public key.
    pub fn new(source: Address, signature: Signature) -> Self {
        Provenance(source, signature)
    }

    /// who generated this signature
    pub fn source(&self) -> Address {
        self.0.clone()
    }

    /// the actual signature data
    pub fn signature(&self) -> Signature {
        self.1.clone()
    }
}
/// Signature is a wrapper structure for a cryptographic signature
/// it is stored as a string and can be validated as having been signed
/// by the private key associated with a given public key.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Hash, Eq, SerializedBytes)]
pub struct Signature(String);

impl Signature {
    /// generate a fake test signature
    pub fn fake() -> Signature {
        test_signature()
    }
}

impl From<&'static str> for Signature {
    fn from(s: &str) -> Signature {
        Signature(s.to_owned())
    }
}

impl From<String> for Signature {
    fn from(s: String) -> Signature {
        Signature(s)
    }
}

/// generate a list of fake test signatures (one entry)
pub fn test_signatures() -> Vec<Signature> {
    vec![test_signature()]
}

/// generate a fake test signature
pub fn test_signature() -> Signature {
    Signature::from("fake-signature")
}

/// generate a different fake test signature
pub fn test_signature_b() -> Signature {
    Signature::from("another-fake-signature")
}

/// generate yet another fake test signature
pub fn test_signature_c() -> Signature {
    Signature::from("sig-c")
}

impl From<Signature> for String {
    fn from(s: Signature) -> String {
        s.0
    }
}
