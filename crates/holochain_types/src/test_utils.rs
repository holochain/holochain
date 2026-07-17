//! Some common testing helpers.

use crate::dna::wasm::DnaWasm;
use crate::prelude::*;
use holochain_zome_types::prelude::{CreateData, UpdateData};
pub use holochain_zome_types::test_utils::*;
use std::path::PathBuf;

#[warn(missing_docs)]
pub mod chain;

/// A fixture example dna for unit testing.
pub fn fake_dna_file(network_seed: &str) -> DnaFile {
    fake_dna_file_named(network_seed, "test")
}

/// A named dna for unit testing.
pub fn fake_dna_file_named(network_seed: &str, name: &str) -> DnaFile {
    fake_dna_zomes_named(network_seed, name, vec![(name.into(), vec![].into())])
}

/// A fixture example dna for unit testing.
pub fn fake_dna_zomes(network_seed: &str, zomes: Vec<(ZomeName, DnaWasm)>) -> DnaFile {
    fake_dna_zomes_named(network_seed, "test", zomes)
}

/// A named dna for unit testing.
pub fn fake_dna_zomes_named(
    network_seed: &str,
    name: &str,
    zomes: Vec<(ZomeName, DnaWasm)>,
) -> DnaFile {
    let mut dna = DnaDef {
        name: name.to_string(),
        modifiers: DnaModifiers {
            properties: YamlProperties::new(yaml_serde::from_str("p: hi").unwrap())
                .try_into()
                .unwrap(),
            network_seed: network_seed.to_string(),
        },
        integrity_zomes: Vec::new(),
        coordinator_zomes: Vec::new(),
        #[cfg(feature = "unstable-migration")]
        lineage: Default::default(),
    };
    tokio_helper::block_forever_on(async move {
        let mut wasm_code = Vec::new();
        for (zome_name, wasm) in zomes {
            let wasm = crate::dna::wasm::DnaWasmHashed::from_content(wasm).await;
            let (wasm, wasm_hash) = wasm.into_inner();
            dna.integrity_zomes.push((
                zome_name,
                ZomeDef::Wasm(WasmZomeDef {
                    wasm_hash,
                    dependencies: Default::default(),
                })
                .into(),
            ));
            wasm_code.push(wasm);
        }
        DnaFile::new(dna, wasm_code).await
    })
}

/// Save a Dna to a file and return the path and tempdir that contains it
pub async fn write_fake_dna_file(dna: DnaFile) -> anyhow::Result<(PathBuf, tempfile::TempDir)> {
    let bundle = DnaBundle::from_dna_file(dna)?;
    let tmp_dir = tempfile::Builder::new().prefix("fake_dna").tempdir()?;

    let path = tmp_dir.path().join("test-dna.dna");
    tokio::fs::write(&path, bundle.pack()?).await?;

    Ok((path, tmp_dir))
}

/// Keeping with convention if Alice is pubkey 1
/// and bob is pubkey 2 then this helps make test
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

#[allow(missing_docs)]
pub trait ActionRefMut {
    fn author_mut(&mut self) -> &mut AgentPubKey;
    fn action_seq_mut(&mut self) -> Option<&mut u32>;
    fn prev_action_mut(&mut self) -> Option<&mut ActionHash>;
    fn entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)>;
    fn timestamp_mut(&mut self) -> &mut Timestamp;
}

// `Action` is an `ActionHeader` + `ActionData` struct, so every header field is
// a direct projection; only entry data is per-variant.
impl ActionRefMut for Action {
    /// returns a mutable reference to the author
    fn author_mut(&mut self) -> &mut AgentPubKey {
        &mut self.header.author
    }

    /// returns a mutable reference to the sequence ordinal of this action
    fn action_seq_mut(&mut self) -> Option<&mut u32> {
        match self.data {
            // The genesis DNA action's sequence is fixed at 0.
            ActionData::Dna(_) => None,
            _ => Some(&mut self.header.action_seq),
        }
    }

    /// returns the previous action except for the DNA action which doesn't have a previous
    fn prev_action_mut(&mut self) -> Option<&mut ActionHash> {
        self.header.prev_action.as_mut()
    }

    fn entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        match &mut self.data {
            ActionData::Create(CreateData {
                entry_hash,
                entry_type,
            }) => Some((entry_hash, entry_type)),
            ActionData::Update(UpdateData {
                entry_hash,
                entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            _ => None,
        }
    }

    /// returns a mutable reference to the timestamp
    fn timestamp_mut(&mut self) -> &mut Timestamp {
        &mut self.header.timestamp
    }
}
