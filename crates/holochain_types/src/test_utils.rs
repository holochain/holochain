//! Some common testing helpers.

use crate::dna::wasm::DnaWasm;
use crate::element::SignedHeaderHashedExt;
use crate::fixt::*;
use crate::prelude::*;
use holochain_keystore::MetaLairClient;
use std::path::PathBuf;

pub use holochain_zome_types::test_utils::*;

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
struct FakeProperties {
    test: String,
}

/// A fixture example dna for unit testing.
pub fn fake_dna_file(uid: &str) -> DnaFile {
    fake_dna_file_named(uid, "test")
}

/// A named dna for unit testing.
pub fn fake_dna_file_named(uid: &str, name: &str) -> DnaFile {
    fake_dna_zomes_named(uid, name, vec![(name.into(), vec![].into())])
}

/// A fixture example dna for unit testing.
pub fn fake_dna_zomes(uid: &str, zomes: Vec<(ZomeName, DnaWasm)>) -> DnaFile {
    fake_dna_zomes_named(uid, "test", zomes)
}

/// A named dna for unit testing.
pub fn fake_dna_zomes_named(uid: &str, name: &str, zomes: Vec<(ZomeName, DnaWasm)>) -> DnaFile {
    let mut dna = DnaDef {
        name: name.to_string(),
        properties: YamlProperties::new(serde_yaml::from_str("p: hi").unwrap())
            .try_into()
            .unwrap(),
        uid: uid.to_string(),
        zomes: Vec::new(),
    };
    tokio_helper::block_forever_on(async move {
        let mut wasm_code = Vec::new();
        for (zome_name, wasm) in zomes {
            let wasm = crate::dna::wasm::DnaWasmHashed::from_content(wasm).await;
            let (wasm, wasm_hash) = wasm.into_inner();
            dna.zomes
                .push((zome_name, ZomeDef::Wasm(WasmZome { wasm_hash })));
            wasm_code.push(wasm);
        }
        DnaFile::new(dna, wasm_code).await
    })
    .unwrap()
}

/// Save a Dna to a file and return the path and tempdir that contains it
pub async fn write_fake_dna_file(dna: DnaFile) -> anyhow::Result<(PathBuf, tempdir::TempDir)> {
    let bundle = DnaBundle::from_dna_file(dna).await?;
    let tmp_dir = tempdir::TempDir::new("fake_dna")?;
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

/// Create a fake SignedHeaderHashed and EntryHashed pair with random content
pub async fn fake_unique_element(
    keystore: &MetaLairClient,
    agent_key: AgentPubKey,
    visibility: EntryVisibility,
) -> anyhow::Result<(SignedHeaderHashed, EntryHashed)> {
    let content: SerializedBytes =
        UnsafeBytes::from(nanoid::nanoid!().as_bytes().to_owned()).into();
    let entry = Entry::App(content.try_into().unwrap()).into_hashed();
    let app_entry_type = AppEntryTypeFixturator::new(visibility).next().unwrap();
    let header_1 = Header::Create(Create {
        author: agent_key,
        timestamp: Timestamp::now(),
        header_seq: 0,
        prev_header: fake_header_hash(1),

        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry.as_hash().to_owned(),
    });

    Ok((
        SignedHeaderHashed::new(&keystore, header_1.into_hashed()).await?,
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
