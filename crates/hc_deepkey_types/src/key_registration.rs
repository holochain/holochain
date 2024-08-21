use hdi::prelude::*;

use crate::{Authorization, KeyAnchor};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct KeyGeneration {
    pub new_key: AgentPubKey,

    // The private key has signed the deepkey agent key to prove ownership
    pub new_key_signing_of_author: Signature,
    // TODO
    // generator: ActionHash, // This is the key authorized to generate new keys on this chain
    // generator_signature: Signature, // The generator key signing the new key
}

impl KeyGeneration {
    pub fn new(key: AgentPubKey, signature: Signature) -> Self {
        Self {
            new_key: key,
            new_key_signing_of_author: signature,
        }
    }
}

impl From<(AgentPubKey, Signature)> for KeyGeneration {
    fn from((key, signature): (AgentPubKey, Signature)) -> Self {
        Self::new(key, signature)
    }
}

impl From<(&AgentPubKey, &Signature)> for KeyGeneration {
    fn from((key, signature): (&AgentPubKey, &Signature)) -> Self {
        (key.to_owned(), signature.to_owned()).into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct KeyRevocation {
    pub prior_key_registration: ActionHash,
    pub revocation_authorization: Vec<Authorization>,
}

impl KeyRevocation {
    pub fn new(prior_key: ActionHash, authorizations: Vec<Authorization>) -> Self {
        Self {
            prior_key_registration: prior_key,
            revocation_authorization: authorizations,
        }
    }
}

impl From<(ActionHash, Vec<Authorization>)> for KeyRevocation {
    fn from((prior_key, authorizations): (ActionHash, Vec<Authorization>)) -> Self {
        Self::new(prior_key, authorizations)
    }
}

impl From<(&ActionHash, &Vec<Authorization>)> for KeyRevocation {
    fn from((prior_key, authorizations): (&ActionHash, &Vec<Authorization>)) -> Self {
        (prior_key.to_owned(), authorizations.to_owned()).into()
    }
}

#[hdk_entry_helper]
#[derive(Clone)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub enum KeyRegistration {
    // Creates a key under management of current KSR on this chain
    Create(KeyGeneration),

    // Unmanaged key. Keys for hosted web users may be of this type, cannot replace/revoke
    CreateOnly(KeyGeneration),

    // Revokes a key and replaces it with a newly generated one
    Update(KeyRevocation, KeyGeneration),

    // Permanently revokes a key (Note: still uses an update action.)
    Delete(KeyRevocation),
}

impl KeyRegistration {
    pub fn key_anchor(&self) -> ExternResult<KeyAnchor> {
        match self {
            KeyRegistration::Create(key_gen) => key_gen.new_key.to_owned(),
            KeyRegistration::CreateOnly(key_gen) => key_gen.new_key.to_owned(),
            KeyRegistration::Update(_, key_gen) => key_gen.new_key.to_owned(),
            KeyRegistration::Delete(_) => Err(wasm_error!(WasmErrorInner::Guest(
                "Cannot derive KeyAnchor from a KeyRegistration::Delete".to_string()
            )))?,
        }
        .try_into()
    }

    pub fn key_anchor_hash(&self) -> ExternResult<EntryHash> {
        hash_entry(self.key_anchor()?)
    }
}
