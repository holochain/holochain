use crate::{entry::Entry, error::HcResult};

use holochain_persistence_api::cas::content::{Address, AddressableContent, Content};

use holochain_json_api::{
    error::{JsonError, JsonResult},
    json::JsonString,
};

use std::{convert::TryFrom, str};

use hcid::*;

pub type Base32 = String;

/// AgentId represents an agent in the Holochain framework.
/// This data struct is meant be stored in the CAS and source-chain.
/// Its key is the public signing key, and is also used as its address.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, DefaultJson, Eq)]
pub struct AgentId {
    /// a nickname for referencing this agent
    pub nick: String,
    /// the encoded public signing key of this agent (the magnifier)
    pub pub_sign_key: Base32,
    // TODO: Add the encoded public encrypting key (the safe / padlock)
    // pub pub_enc_key: Base32,
}

impl AgentId {
    /// A well-known key useful for testing and used by generate_fake()
    pub const FAKE_RAW_KEY: [u8; 32] = [
        42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    /// generate an agent id with fake key
    pub fn generate_fake(nick: &str) -> Self {
        AgentId::new_with_raw_key(nick, str::from_utf8(&AgentId::FAKE_RAW_KEY).unwrap())
            .expect("AgentId fake key generation failed")
    }

    /// initialize an Agent struct with `nick` and `key` that will be encoded with HCID.
    pub fn new_with_raw_key(nick: &str, key: &str) -> HcResult<Self> {
        let codec = HcidEncoding::with_kind("hcs0")?;
        let key_b32 = codec.encode(key.as_bytes())?;
        Ok(AgentId::new(nick, key_b32))
    }

    /// initialize an Agent struct with `nick` and a HCID encoded key.
    pub fn new(nick: &str, key_b32: Base32) -> Self {
        AgentId {
            nick: nick.to_string(),
            pub_sign_key: key_b32,
        }
    }

    /// Get the key decoded with HCID
    pub fn decoded_key(&self) -> HcResult<String> {
        let codec = HcidEncoding::with_kind("hcs0")?;
        let key_b32 = codec.decode(&self.pub_sign_key)?;
        Ok(str::from_utf8(&key_b32).unwrap().to_owned())
    }
}

impl AddressableContent for AgentId {
    /// for an Agent, the address is their public base32 encoded public signing key string
    fn address(&self) -> Address {
        self.pub_sign_key.clone().into()
    }

    /// get the entry content
    fn content(&self) -> Content {
        Entry::AgentId(self.to_owned()).into()
    }

    // build from entry content
    fn try_from_content(content: &Content) -> JsonResult<Self> {
        match Entry::try_from(content)? {
            Entry::AgentId(agent_id) => Ok(agent_id),
            _ => Err(JsonError::SerializationError(
                "Attempted to load AgentId from non AgentID entry".into(),
            )),
        }
    }
}

pub static GOOD_ID: &str = "HcScIkRaAaaaaaaaaaAaaaAAAAaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa";
pub static BAD_ID: &str = "HcScIkRaAaaaaaaaaaAaaaBBBBaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa";
pub static TOO_BAD_ID: &str = "HcScIkRaAaaaaaaaaaBBBBBBBBaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa";

pub fn test_agent_id() -> AgentId {
    AgentId::new("bob", GOOD_ID.to_string())
}

pub fn test_agent_id_with_name(name: &str) -> AgentId {
    AgentId::new(name, name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn test_identity_value() -> Content {
        Content::from_json(&format!(
            "{{\"nick\":\"bob\",\"pub_sign_key\":\"{}\"}}",
            GOOD_ID
        ))
    }

    #[test]
    fn it_can_generate_fake() {
        let agent_id = AgentId::generate_fake("sandwich");
        assert_eq!(
            "HcScIkRaAaaaaaaaaaAaaaAAAAaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa".to_string(),
            agent_id.address().to_string(),
        );
    }

    #[test]
    fn it_should_decode_key() {
        let agent_id = test_agent_id();
        let raw_key = agent_id.decoded_key().unwrap();
        println!("decoded key = {}", raw_key);
    }

    #[test]
    fn it_should_correct_errors() {
        let corrected_id = AgentId::new("bob", BAD_ID.to_string());
        let raw_key = corrected_id.decoded_key().unwrap();
        assert_eq!(test_agent_id().decoded_key().unwrap(), raw_key);
    }

    #[test]
    fn it_fails_if_too_many_errors() {
        let corrected_id = AgentId::new("bob", TOO_BAD_ID.to_string());
        let maybe_key = corrected_id.decoded_key();
        assert!(maybe_key.is_err());
    }

    #[test]
    /// show ToString implementation for Agent
    fn agent_to_string_test() {
        assert_eq!(test_identity_value(), test_agent_id().into());
    }

    #[test]
    /// show AddressableContent implementation for Agent
    fn agent_addressable_content_test() {
        let expected_content =
            Content::from_json("{\"AgentId\":{\"nick\":\"bob\",\"pub_sign_key\":\"HcScIkRaAaaaaaaaaaAaaaAAAAaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa\"}}");
        // content()
        assert_eq!(expected_content, test_agent_id().content(),);

        // from_content()
        assert_eq!(
            test_agent_id(),
            AgentId::try_from_content(&expected_content).unwrap(),
        );
    }
}
