//! agent module

use crate::address::Address;
use crate::address::Addressable;
use holochain_serialized_bytes::prelude::*;
use std::str;

#[cfg(test)]
use sx_fixture::*;

/// AgentId represents an agent in the Holochain framework.
/// This data struct is meant be stored in the CAS and source-chain.
/// Its key is the public signing key, and is also used as its address.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, SerializedBytes, Eq, Hash)]
pub struct AgentId {
    /// a nickname for referencing this agent
    nick: String,
    /// the encoded public signing key of this agent (the magnifier)
    pub_sign_key: Address,
    // TODO: Add the encoded public encrypting key (the safe / padlock)
    // pub pub_enc_key: Base32,
}

impl AgentId {
    /// A well-known key useful for testing and used by generate_fake()
    pub const FAKE_RAW_KEY: [u8; 32] = [
        42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    // TODO implement outside wasm
    // /// generate an agent id with fake key
    // pub fn generate_fake(nick: &str) -> Self {
    //     AgentId::new_with_raw_key(nick, str::from_utf8(&AgentId::FAKE_RAW_KEY).unwrap())
    //         .expect("AgentId fake key generation failed")
    // }

    // @TODO implement outside wasm
    // /// initialize an Agent struct with `nick` and `key` that will be encoded with HCID.
    // pub fn new_with_raw_key(nick: &str, key: &str) -> SkunkResult<Self> {
    //     let codec = HcidEncoding::with_kind("hcs0")?;
    //     let key_b32 = codec.encode(key.as_bytes())?;
    //     Ok(AgentId::new(nick, key_b32))
    // }

    /// initialize an Agent struct with `nick` and a HCID encoded key.
    pub fn new(nick: &str, key: Address) -> Self {
        AgentId {
            nick: nick.to_string(),
            pub_sign_key: key,
        }
    }

    /// Agent nick-name
    pub fn nick(&self) -> &String {
        &self.nick
    }

    /// public signing key
    pub fn pub_sign_key(&self) -> &Address {
        &self.pub_sign_key
    }
}

impl Addressable for AgentId {
    /// for an Agent, the address is their public base32 encoded public signing key string
    fn address(&self) -> Address {
        self.pub_sign_key.clone().into()
    }
}

// should these not be in the tests module?!?

/// Valid test agent id
pub static GOOD_ID: &[u8] = &[1, 2, 3];
/// Invalid test agent id
pub static BAD_ID: &[u8] = &[3, 4, 5];
/// Invalid test agent id #2
pub static TOO_BAD_ID: &[u8] = &[9, 10, 11];

// /// get a named test agent id
// pub fn test_agent_id_with_name(name: &str) -> AgentId {
//     AgentId::new(name, name.to_string())
// }

#[cfg(test)]
impl Fixture for AgentId {
    type Input = ();
    fn fixture(fixture_type: FixtureType<Self::Input>) -> Self {
        match fixture_type {
            FixtureType::A => AgentId::new("bob", Address::new(GOOD_ID.to_vec())),
            _ => unimplemented!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sx_fixture::FixtureType;

    pub fn test_identity_value() -> SerializedBytes {
        SerializedBytes::try_from(AgentId {
            nick: "bob".to_string(),
            pub_sign_key: Address::new(GOOD_ID.to_vec()),
        })
        .unwrap()
    }

    // #[test]
    // fn it_should_correct_errors() {
    //     let corrected_id = AgentId::new("bob", Address::new(BAD_ID.to_vec()));
    //     let raw_key = corrected_id.decoded_key().unwrap();
    //     assert_eq!(
    //         AgentId::fixture(FixtureType::A).decoded_key().unwrap(),
    //         raw_key
    //     );
    // }

    // #[test]
    // fn it_fails_if_too_many_errors() {
    //     let corrected_id = AgentId::new("bob", TOO_BAD_ID.to_string());
    //     let maybe_key = corrected_id.decoded_key();
    //     assert!(maybe_key.is_err());
    // }

    #[test]
    /// show ToString implementation for Agent
    fn agent_to_string_test() {
        assert_eq!(
            test_identity_value(),
            AgentId::fixture(FixtureType::A).try_into().unwrap()
        );
    }
}
