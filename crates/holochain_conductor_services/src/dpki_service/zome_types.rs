//! Mirrors input and output types from the deepkey DNA

use holochain_types::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyState {
    NotFound,
    Invalidated(SignedActionHashed),
    Valid(SignedActionHashed),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyGeneration {
    pub new_key: AgentPubKey,

    // The private key has signed the deepkey agent key to prove ownership
    pub new_key_signing_of_author: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBindingInput {
    pub app_name: String,
    pub installed_app_id: String,
    pub dna_hashes: Vec<DnaHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationDetailsInput {
    pub app_index: u32,
    pub key_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
