pub mod curve;

use crate::conductor::delete_me_create_test_keystore;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsHostAccess;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::init::InitHostAccess;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentHostAccess;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentInvocation;
use crate::core::ribosome::guest_callback::post_commit::PostCommitHostAccess;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateHostAccess;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageHostAccess;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostAccess;
use crate::core::ribosome::ZomeCallHostAccess;
use crate::core::state::metadata::LinkMetaVal;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use fixt::prelude::*;
use holo_hash::DnaHashFixturator;
use holo_hash::HeaderHashFixturator;
use holo_hash::HoloHashExt;
use holo_hash::WasmHash;
use holo_hash_core::HeaderHash;
use holochain_keystore::keystore_actor::KeystoreSender;
use holochain_p2p::HolochainP2pCellFixturator;
use holochain_types::composite_hash::EntryHash;
use holochain_types::dna::wasm::DnaWasm;
use holochain_types::dna::zome::Zome;
use holochain_types::dna::DnaFile;
use holochain_types::dna::Wasms;
use holochain_types::dna::Zomes;
pub use holochain_types::fixt::*;
use holochain_types::test_utils::fake_dna_zomes;
use holochain_wasm_test_utils::strum::IntoEnumIterator;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::header::HeaderHashes;
use holochain_zome_types::link::LinkTag;
use holochain_zome_types::HostInput;
use rand::seq::IteratorRandom;
use rand::thread_rng;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicPtr;
use std::sync::Arc;

wasm_io_fixturator!(HostInput<SerializedBytes>);

newtype_fixturator!(FnComponents<Vec<String>>);

fixturator!(
    WasmRibosome;
    constructor fn new(DnaFile);
);

impl Iterator for WasmRibosomeFixturator<curve::Zomes> {
    type Item = WasmRibosome;

    fn next(&mut self) -> Option<Self::Item> {
        // @todo fixturate this
        let dna_file = fake_dna_zomes(
            &StringFixturator::new(Unpredictable).next().unwrap(),
            self.0
                .curve
                .0
                .clone()
                .into_iter()
                .map(|t| (t.into(), t.into()))
                .collect(),
        );

        let ribosome = WasmRibosome::new(dna_file);

        // warm the module cache for each wasm in the ribosome
        for zome in self.0.curve.0.clone() {
            let mut host_context = CallContextFixturator::new(Empty).next().unwrap();
            host_context.zome_name = zome.into();
            ribosome.module(host_context).unwrap();
        }

        self.0.index += 1;

        Some(ribosome)
    }
}

fixturator!(
    DnaWasm;
    // note that an empty wasm will not compile
    curve Empty DnaWasm { code: Arc::new(vec![]) };
    curve Unpredictable TestWasm::iter().choose(&mut thread_rng()).unwrap().into();
    curve Predictable TestWasm::iter().cycle().nth(self.0.index).unwrap().into();
);

fixturator!(
    Wasms;
    curve Empty BTreeMap::new();
    curve Unpredictable {
        let mut rng = rand::thread_rng();
        let number_of_wasms = rng.gen_range(0, 5);

        let mut wasms: Wasms = BTreeMap::new();
        let mut dna_wasm_fixturator = DnaWasmFixturator::new(Unpredictable);
        for _ in (0..number_of_wasms) {
            let wasm = dna_wasm_fixturator.next().unwrap();
            wasms.insert(
                tokio_safe_block_on::tokio_safe_block_forever_on(
                    async { WasmHash::with_data(wasm.code().to_vec()).await },
                )
                .into(),
                wasm,
            );
        }
        wasms
    };
    curve Predictable {
        let mut wasms: Wasms = BTreeMap::new();
        let mut dna_wasm_fixturator = DnaWasmFixturator::new_indexed(Predictable, self.0.index);
        for _ in (0..3) {
            let wasm = dna_wasm_fixturator.next().unwrap();
            wasms.insert(
                tokio_safe_block_on::tokio_safe_block_forever_on(
                    async { WasmHash::with_data(wasm.code().to_vec()).await },
                )
                .into(),
                wasm,
            );
        }
        wasms
    };
);

fixturator!(
    DnaFile,
    {
        DnaFile {
            dna: DnaDefFixturator::new(Empty).next().unwrap(),
            dna_hash: DnaHashFixturator::new(Empty).next().unwrap(),
            code: WasmsFixturator::new(Empty).next().unwrap(),
        }
    },
    {
        // align the wasm hashes across the file and def
        let mut zome_name_fixturator = ZomeNameFixturator::new(Unpredictable);
        let wasms = WasmsFixturator::new(Unpredictable).next().unwrap();
        let mut zomes: Zomes = Vec::new();
        for (hash, wasm) in wasms {
            zomes.push((
                zome_name_fixturator.next().unwrap(),
                Zome {
                    wasm_hash: hash.to_owned(),
                },
            ));
        }
        let mut dna_def = DnaDefFixturator::new(Unpredictable).next().unwrap();
        dna_def.zomes = zomes;
        DnaFile {
            dna: dna_def,
            dna_hash: DnaHashFixturator::new(Unpredictable).next().unwrap(),
            code: WasmsFixturator::new(Unpredictable).next().unwrap(),
        }
    },
    {
        // align the wasm hashes across the file and def
        let mut zome_name_fixturator = ZomeNameFixturator::new_indexed(Predictable, self.0.index);
        let wasms = WasmsFixturator::new_indexed(Predictable, self.0.index)
            .next()
            .unwrap();
        let mut zomes: Zomes = Vec::new();
        for (hash, wasm) in wasms {
            zomes.push((
                zome_name_fixturator.next().unwrap(),
                Zome {
                    wasm_hash: hash.to_owned(),
                },
            ));
        }
        let mut dna_def = DnaDefFixturator::new_indexed(Predictable, self.0.index)
            .next()
            .unwrap();
        dna_def.zomes = zomes;
        DnaFile {
            dna: DnaDefFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            dna_hash: DnaHashFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            code: WasmsFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        }
    }
);

fixturator!(
    LinkMetaVal;
    constructor fn new(HeaderHash, EntryHash, Timestamp, u8, LinkTag);
);

impl Iterator for LinkMetaValFixturator<(EntryHash, LinkTag)> {
    type Item = LinkMetaVal;
    fn next(&mut self) -> Option<Self::Item> {
        let mut f = fixt!(LinkMetaVal);
        f.target = self.0.curve.0.clone();
        f.tag = self.0.curve.1.clone();
        Some(f)
    }
}

fixturator!(
    HeaderHashes,
    vec![].into(),
    {
        let mut rng = rand::thread_rng();
        let number_of_hashes = rng.gen_range(0, 5);

        let mut hashes: Vec<HeaderHash> = vec![];
        let mut header_hash_fixturator = HeaderHashFixturator::new(Unpredictable);
        for _ in (0..number_of_hashes) {
            hashes.push(header_hash_fixturator.next().unwrap().into());
        }
        hashes.into()
    },
    {
        let mut hashes: Vec<HeaderHash> = vec![];
        let mut header_hash_fixturator =
            HeaderHashFixturator::new_indexed(Predictable, self.0.index);
        for _ in 0..3 {
            hashes.push(header_hash_fixturator.next().unwrap().into());
        }
        hashes.into()
    }
);

fixturator!(
    KeystoreSender;
    curve Empty {
        tokio_safe_block_on::tokio_safe_block_forever_on(async {
            let _ = holochain_crypto::crypto_init_sodium();
            delete_me_create_test_keystore().await
        })
    };
    curve Unpredictable {
        // TODO: Make this unpredictable
        tokio_safe_block_on::tokio_safe_block_forever_on(async {
            let _ = holochain_crypto::crypto_init_sodium();
            delete_me_create_test_keystore().await
        })
    };
    curve Predictable {
        tokio_safe_block_on::tokio_safe_block_forever_on(async {
            let _ = holochain_crypto::crypto_init_sodium();
            delete_me_create_test_keystore().await
        })
    };
);

fixturator!(
    UnsafeInvokeZomeWorkspace;
    curve Empty {
        UnsafeInvokeZomeWorkspace::null()
    };
    curve Unpredictable {
        UnsafeInvokeZomeWorkspaceFixturator::new(Empty)
            .next()
            .unwrap()
    };
    curve Predictable {
        UnsafeInvokeZomeWorkspaceFixturator::new(Empty)
            .next()
            .unwrap()
    };
);

fixturator!(
    ZomeCallHostAccess;
    constructor fn new(UnsafeInvokeZomeWorkspace, KeystoreSender, HolochainP2pCell);
);

fixturator!(
    EntryDefsInvocation;
    constructor fn new();
);

fixturator!(
    EntryDefsHostAccess;
    constructor fn new();
);

fixturator!(
    InitInvocation;
    constructor fn new(DnaDef);
);

fixturator!(
    InitHostAccess;
    constructor fn new(UnsafeInvokeZomeWorkspace, KeystoreSender, HolochainP2pCell);
);

fixturator!(
    MigrateAgentInvocation;
    constructor fn new(DnaDef, MigrateAgent);
);

fixturator!(
    MigrateAgentHostAccess;
    constructor fn new(UnsafeInvokeZomeWorkspace);
);

fixturator!(
    PostCommitInvocation;
    constructor fn new(ZomeName, HeaderHashes);
);

fixturator!(
    PostCommitHostAccess;
    constructor fn new(UnsafeInvokeZomeWorkspace, KeystoreSender, HolochainP2pCell);
);

fixturator!(
    ValidateInvocation;
    constructor fn new(ZomeName, Entry);
);

fixturator!(
    ValidateHostAccess;
    constructor fn new();
);

fixturator!(
    ValidationPackageInvocation;
    constructor fn new(ZomeName, AppEntryType);
);

fixturator!(
    ValidationPackageHostAccess;
    constructor fn new(UnsafeInvokeZomeWorkspace);
);

fixturator!(
    HostAccess;
    variants [
        ZomeCall(ZomeCallHostAccess)
        Validate(ValidateHostAccess)
        Init(InitHostAccess)
        EntryDefs(EntryDefsHostAccess)
        MigrateAgent(MigrateAgentHostAccess)
        ValidationPackage(ValidationPackageHostAccess)
        PostCommit(PostCommitHostAccess)
    ];
);

fixturator!(
    CallContext;
    constructor fn new(ZomeName, HostAccess);
);
