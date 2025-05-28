//! Some common testing helpers.

use crate::dna::wasm::DnaWasm;
use crate::prelude::*;
use crate::record::SignedActionHashedExt;
use holochain_keystore::MetaLairClient;
pub use holochain_zome_types::test_utils::*;
use std::path::PathBuf;

#[warn(missing_docs)]
pub mod chain;

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
struct FakeProperties {
    test: String,
}

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
            properties: YamlProperties::new(serde_yaml::from_str("p: hi").unwrap())
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
                ZomeDef::Wasm(WasmZome {
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

/// Create a fake SignedActionHashed and EntryHashed pair with random content
pub async fn fake_unique_record(
    keystore: &MetaLairClient,
    agent_key: AgentPubKey,
    visibility: EntryVisibility,
) -> anyhow::Result<(SignedActionHashed, EntryHashed)> {
    let content: SerializedBytes =
        UnsafeBytes::from(nanoid::nanoid!().as_bytes().to_owned()).into();
    let entry = Entry::App(content.try_into().unwrap()).into_hashed();
    let app_entry_def = AppEntryDefFixturator::new(visibility).next().unwrap();
    let action_1 = Action::Create(Create {
        author: agent_key,
        timestamp: Timestamp::now(),
        action_seq: 0,
        prev_action: fake_action_hash(1),

        entry_type: EntryType::App(app_entry_def),
        entry_hash: entry.as_hash().to_owned(),

        weight: Default::default(),
    });

    Ok((
        SignedActionHashed::sign(keystore, action_1.into_hashed()).await?,
        entry,
    ))
}

#[allow(missing_docs)]
pub trait ActionRefMut {
    fn author_mut(&mut self) -> &mut AgentPubKey;
    fn action_seq_mut(&mut self) -> Option<&mut u32>;
    fn prev_action_mut(&mut self) -> Option<&mut ActionHash>;
    fn entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)>;
    fn timestamp_mut(&mut self) -> &mut Timestamp;
}

impl ActionRefMut for Action {
    /// returns a mutable reference to the author
    fn author_mut(&mut self) -> &mut AgentPubKey {
        match *self {
            Self::Dna(Dna { ref mut author, .. })
            | Self::AgentValidationPkg(AgentValidationPkg { ref mut author, .. })
            | Self::InitZomesComplete(InitZomesComplete { ref mut author, .. })
            | Self::CreateLink(CreateLink { ref mut author, .. })
            | Self::DeleteLink(DeleteLink { ref mut author, .. })
            | Self::Delete(Delete { ref mut author, .. })
            | Self::CloseChain(CloseChain { ref mut author, .. })
            | Self::OpenChain(OpenChain { ref mut author, .. })
            | Self::Create(Create { ref mut author, .. })
            | Self::Update(Update { ref mut author, .. }) => author,
        }
    }

    /// returns a mutable reference to the sequence ordinal of this action
    fn action_seq_mut(&mut self) -> Option<&mut u32> {
        match *self {
            // Dna is always 0
            Self::Dna(Dna { .. }) => None,
            Self::AgentValidationPkg(AgentValidationPkg {
                ref mut action_seq, ..
            })
            | Self::InitZomesComplete(InitZomesComplete {
                ref mut action_seq, ..
            })
            | Self::CreateLink(CreateLink {
                ref mut action_seq, ..
            })
            | Self::DeleteLink(DeleteLink {
                ref mut action_seq, ..
            })
            | Self::Delete(Delete {
                ref mut action_seq, ..
            })
            | Self::CloseChain(CloseChain {
                ref mut action_seq, ..
            })
            | Self::OpenChain(OpenChain {
                ref mut action_seq, ..
            })
            | Self::Create(Create {
                ref mut action_seq, ..
            })
            | Self::Update(Update {
                ref mut action_seq, ..
            }) => Some(action_seq),
        }
    }

    /// returns the previous action except for the DNA action which doesn't have a previous
    fn prev_action_mut(&mut self) -> Option<&mut ActionHash> {
        match self {
            Self::Dna(Dna { .. }) => None,
            Self::AgentValidationPkg(AgentValidationPkg {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::InitZomesComplete(InitZomesComplete {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::CreateLink(CreateLink {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::DeleteLink(DeleteLink {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::Delete(Delete {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::CloseChain(CloseChain {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::OpenChain(OpenChain {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::Create(Create {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::Update(Update {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
        }
    }

    fn entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        match self {
            Self::Create(Create {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            Self::Update(Update {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            _ => None,
        }
    }

    /// returns a mutable reference to the timestamp
    fn timestamp_mut(&mut self) -> &mut Timestamp {
        match *self {
            Self::Dna(Dna {
                ref mut timestamp, ..
            })
            | Self::AgentValidationPkg(AgentValidationPkg {
                ref mut timestamp, ..
            })
            | Self::InitZomesComplete(InitZomesComplete {
                ref mut timestamp, ..
            })
            | Self::CreateLink(CreateLink {
                ref mut timestamp, ..
            })
            | Self::DeleteLink(DeleteLink {
                ref mut timestamp, ..
            })
            | Self::Delete(Delete {
                ref mut timestamp, ..
            })
            | Self::CloseChain(CloseChain {
                ref mut timestamp, ..
            })
            | Self::OpenChain(OpenChain {
                ref mut timestamp, ..
            })
            | Self::Create(Create {
                ref mut timestamp, ..
            })
            | Self::Update(Update {
                ref mut timestamp, ..
            }) => timestamp,
        }
    }
}

impl ActionRefMut for NewEntryAction {
    fn author_mut(&mut self) -> &mut AgentPubKey {
        match self {
            Self::Create(Create { ref mut author, .. }) => author,
            Self::Update(Update { ref mut author, .. }) => author,
        }
    }

    fn action_seq_mut(&mut self) -> Option<&mut u32> {
        Some(match self {
            Self::Create(Create {
                ref mut action_seq, ..
            }) => action_seq,
            Self::Update(Update {
                ref mut action_seq, ..
            }) => action_seq,
        })
    }

    fn prev_action_mut(&mut self) -> Option<&mut ActionHash> {
        todo!()
    }

    fn entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        Some(match self {
            Self::Create(Create {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => (entry_hash, entry_type),
            Self::Update(Update {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => (entry_hash, entry_type),
        })
    }

    fn timestamp_mut(&mut self) -> &mut Timestamp {
        match self {
            Self::Create(Create {
                ref mut timestamp, ..
            }) => timestamp,
            Self::Update(Update {
                ref mut timestamp, ..
            }) => timestamp,
        }
    }
}

/// Create test chain data
pub async fn valid_arbitrary_chain(
    keystore: &MetaLairClient,
    author: AgentPubKey,
    length: usize,
) -> Vec<Record> {
    use ::fixt::*;
    use holo_hash::fixt::DnaHashFixturator;

    let mut out = Vec::new();
    let extend_out = |mut out: Vec<Record>, action: Action, entry: Option<Entry>| async move {
        out.push(Record::new(
            SignedActionHashed::sign(keystore, action.into_hashed())
                .await
                .unwrap(),
            entry,
        ));
        out
    };

    let dna = Action::Dna(Dna {
        author: author.clone(),
        hash: fixt!(DnaHash),
        timestamp: Timestamp::now(),
    });
    out = extend_out(out, dna, None).await;

    let avp = Action::AgentValidationPkg(AgentValidationPkg {
        author: author.clone(),
        timestamp: Timestamp::now(),
        action_seq: 1,
        prev_action: out.last().as_ref().unwrap().action_address().clone(),
        membrane_proof: None,
    });
    out = extend_out(out, avp, None).await;

    let agent_entry = Entry::Agent(author.clone());

    let agent = Action::Create(Create {
        author: author.clone(),
        timestamp: Timestamp::now(),
        action_seq: 2,
        prev_action: out.last().as_ref().unwrap().action_address().clone(),
        entry_type: EntryType::AgentPubKey,
        entry_hash: agent_entry.clone().into_hashed().hash,
        weight: Default::default(),
    });
    out = extend_out(out, agent, Some(agent_entry)).await;

    let init_zomes = Action::InitZomesComplete(InitZomesComplete {
        author: author.clone(),
        timestamp: Timestamp::now(),
        action_seq: 3,
        prev_action: out.last().as_ref().unwrap().action_address().clone(),
    });
    out = extend_out(out, init_zomes, None).await;

    for action_seq in 4..length {
        let entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
            nanoid::nanoid!().as_bytes().to_owned(),
        ))));

        let action = Action::Create(Create {
            author: author.clone(),
            timestamp: Timestamp::now(),
            action_seq: action_seq as u32,
            prev_action: out.last().as_ref().unwrap().action_address().clone(),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                1.into(),
                EntryVisibility::Public,
            )),
            entry_hash: entry.clone().into_hashed().hash,
            weight: Default::default(),
        });
        out = extend_out(out, action, Some(entry)).await;
    }

    out
}
