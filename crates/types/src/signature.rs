//! The Signature type is defined here. They are used in ChainHeaders as
//! a way of providing cryptographically verifiable proof of a given agent
//! as having been the author of a given data entry.

use holochain_persistence_api::cas::content::Address;

use holochain_json_api::{error::JsonError, json::JsonString};

/// Provenance is a tuple of initiating agent public key and signature of some item being signed
/// this type is used in headers and in capability requests where the item being signed
/// is implicitly known by context
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, DefaultJson)]
pub struct Provenance(pub Address, pub Signature);

impl Provenance {
    /// Creates a new provenance instance with source typically
    /// being an agent address (public key) and the signature
    /// some signed data using the private key associated with
    /// the public key.
    pub fn new(source: Address, signature: Signature) -> Self {
        Provenance(source, signature)
    }
    pub fn source(&self) -> Address {
        self.0.clone()
    }
    pub fn signature(&self) -> Signature {
        self.1.clone()
    }
}
/// Signature is a wrapper structure for a cryptographic signature
/// it is stored as a string and can be validated as having been signed
/// by the private key associated with a given public key.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, DefaultJson)]
pub struct Signature(String);

impl Signature {
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

pub fn test_signatures() -> Vec<Signature> {
    vec![test_signature()]
}

pub fn test_signature() -> Signature {
    Signature::from("fake-signature")
}

pub fn test_signature_b() -> Signature {
    Signature::from("another-fake-signature")
}

pub fn test_signature_c() -> Signature {
    Signature::from("sig-c")
}

impl From<Signature> for String {
    fn from(s: Signature) -> String {
        s.0
    }
}
