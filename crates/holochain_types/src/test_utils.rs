//! Some common testing helpers.

use crate::dna::wasm::DnaWasm;
use crate::fixt::*;
use crate::prelude::*;
use crate::record::SignedActionHashedExt;
use holochain_keystore::MetaLairClient;
use std::path::PathBuf;

pub use holochain_zome_types::test_utils::*;

#[allow(missing_docs)]
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
        properties: YamlProperties::new(serde_yaml::from_str("p: hi").unwrap())
            .try_into()
            .unwrap(),
        network_seed: network_seed.to_string(),
        origin_time: Timestamp::HOLOCHAIN_EPOCH,
        integrity_zomes: Vec::new(),
        coordinator_zomes: Vec::new(),
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
    .unwrap()
}

/// Save a Dna to a file and return the path and tempdir that contains it
pub async fn write_fake_dna_file(dna: DnaFile) -> anyhow::Result<(PathBuf, tempfile::TempDir)> {
    let bundle = DnaBundle::from_dna_file(dna).await?;
    let tmp_dir = tempfile::Builder::new()
        .prefix("fake_dna")
        .tempdir()
        .unwrap();
    let mut path: PathBuf = tmp_dir.path().into();
    path.push("test-dna.dna");
    bundle.write_to_file(&path).await?;
    Ok((path, tmp_dir))
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

/// Create a fake SignedActionHashed and EntryHashed pair with random content
pub async fn fake_unique_record(
    keystore: &MetaLairClient,
    agent_key: AgentPubKey,
    visibility: EntryVisibility,
) -> anyhow::Result<(SignedActionHashed, EntryHashed)> {
    let content: SerializedBytes =
        UnsafeBytes::from(nanoid::nanoid!().as_bytes().to_owned()).into();
    let entry = Entry::App(content.try_into().unwrap()).into_hashed();
    let app_entry_type = AppEntryTypeFixturator::new(visibility).next().unwrap();
    let action_1 = Action::Create(Create {
        author: agent_key,
        timestamp: Timestamp::now(),
        action_seq: 0,
        prev_action: fake_action_hash(1),

        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry.as_hash().to_owned(),

        weight: Default::default(),
    });

    Ok((
        SignedActionHashed::sign(keystore, action_1.into_hashed()).await?,
        entry,
    ))
}

/// Generate a test keystore pre-populated with a couple test keypairs.
pub fn test_keystore() -> holochain_keystore::MetaLairClient {
    tokio_helper::block_on(
        async move {
            let keystore = holochain_keystore::test_keystore::spawn_test_keystore()
                .await
                .unwrap();

            keystore
        },
        std::time::Duration::from_secs(1),
    )
    .expect("timeout elapsed")
}

/// The hash type for a [`TestChainItem`]
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Deref,
    derive_more::Into,
)]
pub struct TestChainHash(pub u32);

impl From<u8> for TestChainHash {
    fn from(u: u8) -> Self {
        Self(u as u32)
    }
}

impl From<i32> for TestChainHash {
    fn from(u: i32) -> Self {
        Self(u as u32)
    }
}

impl From<TestChainHash> for ActionHash {
    fn from(_: TestChainHash) -> Self {
        todo!("remove when isotest is merged")
    }
}

/// A test implementation of a minimal ChainItem which uses simple numbers for hashes
/// and always points back to the previous number
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TestChainItem {
    /// The sequence number
    pub seq: u32,
    /// The hash
    pub hash: TestChainHash,
    /// The previous hash, unless this is the first item
    pub prev: Option<TestChainHash>,
}

impl TestChainItem {
    /// Constructor for happy-path chains with no forking
    pub fn new(seq: u32) -> Self {
        Self {
            seq,
            hash: TestChainHash(seq),
            prev: seq.checked_sub(1).map(TestChainHash),
        }
    }
}

impl ChainItem for TestChainItem {
    type Hash = TestChainHash;

    fn seq(&self) -> u32 {
        self.seq
    }

    fn get_hash(&self) -> &Self::Hash {
        &self.hash
    }

    fn prev_hash(&self) -> Option<&Self::Hash> {
        self.prev.as_ref()
    }
}

impl AsRef<Self> for TestChainItem {
    fn as_ref(&self) -> &Self {
        self
    }
}
