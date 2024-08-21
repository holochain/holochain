pub use deepkey_types;
pub use deepkey_types::*;

pub use hdk;

use hdk::prelude::{holo_hash::DnaHash, *};
use serde_bytes::ByteArray;

#[hdk_entry_helper]
#[derive(Clone)]
pub enum KeyState {
    NotFound,
    Invalid(Option<SignedActionHashed>),
    Valid(SignedActionHashed),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRevocationInput {
    pub prior_key_registration: ActionHash,
    pub revocation_authorization: Vec<(u8, ByteArray<64>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationDetails {
    pub app_index: u32,
    pub key_index: u32,
}

impl DerivationDetails {
    pub fn to_derivation_path(&self) -> Vec<u32> {
        vec![self.app_index, self.key_index]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBindingInput {
    pub app_name: String,
    pub installed_app_id: String,
    pub dna_hashes: Vec<DnaHash>,
    #[serde(default)]
    pub metadata: deepkey_types::MetaData,
}

#[cfg(feature = "fuzzing")]
impl<'a> arbitrary::Arbitrary<'a> for AppBindingInput {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            app_name: arbitrary::Arbitrary::arbitrary(u)?,
            installed_app_id: arbitrary::Arbitrary::arbitrary(u)?,
            dna_hashes: arbitrary::Arbitrary::arbitrary(u)?,
            metadata: deepkey_types::MetaData::new(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct DerivationDetailsInput {
    pub app_index: u32,
    pub key_index: u32,
    #[serde(with = "serde_bytes")]
    pub derivation_seed: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub derivation_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct CreateKeyInput {
    pub key_generation: KeyGeneration,
    pub app_binding: AppBindingInput,
    pub derivation_details: Option<DerivationDetailsInput>,
    #[serde(default)]
    pub create_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateKeyInput {
    pub key_revocation: KeyRevocation,
    pub key_generation: KeyGeneration,
    pub derivation_details: Option<DerivationDetailsInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeKeyInput {
    pub key_revocation: KeyRevocation,
}

impl TryFrom<KeyRevocationInput> for KeyRevocation {
    type Error = WasmError;

    fn try_from(input: KeyRevocationInput) -> ExternResult<Self> {
        Ok(Self {
            prior_key_registration: input.prior_key_registration,
            revocation_authorization: input
                .revocation_authorization
                .into_iter()
                .map(|(index, signature)| (index, Signature::from(signature.into_array())))
                .collect(),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthoritySpecInput {
    pub sigs_required: u8,
    pub authorized_signers: Vec<ByteArray<32>>,
}

impl From<AuthoritySpecInput> for AuthoritySpec {
    fn from(input: AuthoritySpecInput) -> Self {
        Self {
            sigs_required: input.sigs_required,
            authorized_signers: input
                .authorized_signers
                .iter()
                .map(|key| key.into_array())
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateChangeRuleInput {
    pub authority_spec: AuthoritySpecInput,
    pub authorizations: Option<Vec<Authorization>>,
}
