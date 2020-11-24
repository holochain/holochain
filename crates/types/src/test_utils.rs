//! Some common testing helpers.

use crate::cell::CellId;
use crate::prelude::*;
use holochain_zome_types::capability::CapSecret;
use holochain_zome_types::capability::CAP_SECRET_BYTES;

pub use holochain_zome_types::test_utils::*;

#[derive(Serialize, Deserialize, SerializedBytes)]
struct FakeProperties {
    test: String,
}

/// A fixture example CellId for unit testing.
pub fn fake_cell_id(name: u8) -> CellId {
    (fake_dna_hash(name), fake_agent_pubkey_1()).into()
}

/// Keeping with convention if Alice is pubkey 1
/// and bob is pubkey 2 the this helps make test
/// logging easier to read.
pub fn which_agent(key: &AgentPubKey) -> String {
    let key = key.to_string();
    let alice = fake_agent_pubkey_1().to_string();
    let bob = fake_agent_pubkey_2().to_string();
    if key == alice {
        return "alice".to_string();
    }
    if key == bob {
        return "bob".to_string();
    }
    key
}

/// A fixture CapSecret for unit testing.
pub fn fake_cap_secret() -> CapSecret {
    [0; CAP_SECRET_BYTES].into()
}
