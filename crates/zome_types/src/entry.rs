use holo_hash_core::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

//TODO move to capabilities module
/// Entry data for a capability claim
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct CapTokenClaim;
/// Entry data for a capability grant
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct CapTokenGrant;

/// Structure holding the entry portion of a chain element.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
#[allow(clippy::large_enum_variant)]
#[serde(tag = "entry_type", content = "entry")]
pub enum Entry {
    /// The `Agent` system entry, the third entry of every source chain,
    /// which grants authoring capability for this agent.
    Agent(AgentPubKey),
    /// The application entry data for entries that aren't system created entries
    App(SerializedBytes),
    /// The capability claim system entry which allows committing a granted permission
    /// for later use
    CapTokenClaim(CapTokenClaim),
    /// The capability grant system entry which allows granting of application defined
    /// capabilities
    CapTokenGrant(CapTokenGrant),
}

make_hashed_base! {
    Visibility(pub),
    HashedName(EntryHashed),
    ContentType(Entry),
    HashType(EntryAddress),
}

impl EntryHashed {
    /// Construct (and hash) a new EntryHashed with given Entry.
    pub async fn with_data(entry: Entry) -> Result<Self, SerializedBytesError> {
        let hash = match &entry {
            Entry::Agent(key) => EntryAddress::Agent(key.to_owned()),
            entry => {
                let sb = SerializedBytes::try_from(entry)?;
                EntryAddress::Entry(EntryHash::with_data(sb.bytes()).await)
            }
        };
        Ok(EntryHashed::with_pre_hashed(entry, hash))
    }
}
