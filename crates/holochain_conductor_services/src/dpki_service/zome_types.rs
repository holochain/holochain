//! Mirrors input and output types from the deepkey DNA

use holochain_types::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub enum KeyState {
    NotFound,
    Invalidated(SignedActionHashed),
    Valid(SignedActionHashed),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct KeyMeta {
    pub app_binding_addr: ActionHash,
    pub key_index: u32,
    pub key_registration_addr: ActionHash,
    pub key_anchor_addr: ActionHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct KeyRevocation {
    pub prior_key_registration: ActionHash,
    pub revocation_authorization: Vec<Authorization>,
}

pub type Authorization = (u8, Signature);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct KeyGeneration {
    pub new_key: AgentPubKey,

    // The private key has signed the deepkey agent key to prove ownership
    pub new_key_signing_of_author: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct AppBindingInput {
    pub app_name: String,
    pub installed_app_id: String,
    pub dna_hashes: Vec<DnaHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct DerivationDetailsInput {
    pub app_index: u32,
    pub key_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct CreateKeyInput {
    pub key_generation: KeyGeneration,
    pub app_binding: AppBindingInput,
    pub derivation_details: DerivationDetailsInput,
}

impl KeyState {
    pub fn is_valid(&self) -> bool {
        matches!(self, KeyState::Valid(_))
    }
}

impl DerivationDetailsInput {
    pub fn to_derivation_path(&self) -> Vec<u32> {
        vec![self.app_index, self.key_index]
    }
}
