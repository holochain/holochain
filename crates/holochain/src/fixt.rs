//! Fixturators for holochain types

use crate::conductor::api::CellConductorReadHandle;
use crate::conductor::api::MockCellConductorReadHandleT;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsHostAccess;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::init::InitHostAccess;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::post_commit::PostCommitHostAccess;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateHostAccess;
#[cfg(feature = "wasmer_sys")]
use crate::core::ribosome::real_ribosome::ModuleCacheLock;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomeCallHostAccess;
use crate::core::ribosome::ZomesToInvoke;
use crate::sweettest::SweetDnaFile;
use crate::test_utils::fake_genesis;
use ::fixt::prelude::*;
pub use holo_hash::fixt::*;
use holo_hash::WasmHash;
use holochain_keystore::test_keystore;
use holochain_keystore::MetaLairClient;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
#[cfg(feature = "wasmer_sys")]
use holochain_wasmer_host::module::ModuleCache;
use rand::rng;
use rand::seq::IteratorRandom;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::Arc;
use strum::IntoEnumIterator;
use tokio::sync::broadcast;

pub use holochain_types::fixt::*;

/// A collection of test WASMs.
pub struct Zomes(pub Vec<TestWasm>);

newtype_fixturator!(FnComponents<Vec<String>>);

fixturator!(
    RealRibosome;
    constructor fn empty(DnaFile);
);

impl Iterator for RealRibosomeFixturator<Zomes> {
    type Item = RealRibosome;

    fn next(&mut self) -> Option<Self::Item> {
        let input = self.0.curve.0.clone();
        let uuid = StringFixturator::new(Unpredictable).next().unwrap();
        let (dna_file, _, _) = tokio_helper::block_forever_on(async move {
            SweetDnaFile::from_test_wasms(uuid, input, Default::default()).await
        });

        #[cfg(feature = "wasmer_wamr")]
        let module_cache = None;
        #[cfg(feature = "wasmer_sys")]
        let module_cache = Some(Arc::new(ModuleCacheLock::new(ModuleCache::new(None))));

        let ribosome =
            tokio_helper::block_forever_on(RealRibosome::new(dna_file, module_cache)).unwrap();

        // warm the module cache for each wasm in the ribosome
        for zome in self.0.curve.0.clone() {
            let mut call_context = CallContextFixturator::new(Empty).next().unwrap();
            call_context.zome = CoordinatorZome::from(zome).erase_type();
            tokio_helper::block_forever_on(ribosome.build_module(call_context.zome.zome_name()))
                .unwrap();
        }

        self.0.index += 1;

        Some(ribosome)
    }
}

fixturator!(
    DnaWasm;
    // note that an empty wasm will not compile
    curve Empty DnaWasm { code: Default::default() };
    curve Unpredictable TestWasm::iter().choose(&mut rng()).unwrap().into();
    curve Predictable TestWasm::iter().cycle().nth(get_fixt_index!()).unwrap().into();
);

fixturator!(
    WasmMap;
    curve Empty BTreeMap::new().into();
    curve Unpredictable {
        let mut rng = rand::rng();
        let number_of_wasms = rng.random_range(0..5);

        let mut wasms = BTreeMap::new();
        let mut dna_wasm_fixturator = DnaWasmFixturator::new(Unpredictable);
        for _ in 0..number_of_wasms {
            let wasm = dna_wasm_fixturator.next().unwrap();
            wasms.insert(
                tokio_helper::block_forever_on(
                    async { WasmHash::with_data(&wasm).await },
                ),
                wasm,
            );
        }
        wasms.into()
    };
    curve Predictable {
        let mut wasms = BTreeMap::new();
        let mut dna_wasm_fixturator = DnaWasmFixturator::new_indexed(Predictable, get_fixt_index!());
        for _ in 0..3 {
            let wasm = dna_wasm_fixturator.next().unwrap();
            wasms.insert(
                tokio_helper::block_forever_on(
                    async { WasmHash::with_data(&wasm).await },
                ),
                wasm,
            );
        }
        wasms.into()
    };
);

fixturator!(
    DnaFile;
    curve Empty {
        DnaFile::from_parts(
            DnaDefFixturator::new(Empty).next().unwrap().into_hashed(),
            WasmMapFixturator::new(Empty).next().unwrap(),
        )
    };
    curve Unpredictable {
        // align the wasm hashes across the file and def
        let mut zome_name_fixturator = ZomeNameFixturator::new(Unpredictable);
        let wasms = WasmMapFixturator::new(Unpredictable).next().unwrap();
        let mut zomes: IntegrityZomes = Vec::new();
        for (hash, _) in wasms {
            zomes.push((
                zome_name_fixturator.next().unwrap(),
                IntegrityZomeDef::from_hash(
                    hash.to_owned()
                ),
            ));
        }
        let mut dna_def = DnaDefFixturator::new(Unpredictable).next().unwrap();
        dna_def.integrity_zomes = zomes;
        let dna = dna_def.into_hashed();
        DnaFile::from_parts(dna, WasmMapFixturator::new(Unpredictable).next().unwrap())
    };
    curve Predictable {
        // align the wasm hashes across the file and def
        let mut zome_name_fixturator =
            ZomeNameFixturator::new_indexed(Predictable, get_fixt_index!());
        let wasms = WasmMapFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap();
        let mut zomes: IntegrityZomes = Vec::new();
        for (hash, _) in wasms {
            zomes.push((
                zome_name_fixturator.next().unwrap(),
                IntegrityZomeDef::from_hash(
                    hash.to_owned()
                ),
            ));
        }
        let mut dna_def = DnaDefFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap();
        dna_def.integrity_zomes = zomes;
        let dna = dna_def.into_hashed();
        DnaFile::from_parts(
            dna,
            WasmMapFixturator::new_indexed(Predictable, get_fixt_index!())
                .next()
                .unwrap(),
        )
    };
);

fixturator!(
    MetaLairClient;
    curve Empty {
        tokio_helper::block_forever_on(async {
            // an empty keystore
            holochain_keystore::spawn_test_keystore().await.unwrap()
        })
    };
    curve Unpredictable {
        // TODO: Make this unpredictable
        tokio_helper::block_forever_on(async {
            holochain_keystore::spawn_test_keystore().await.unwrap()
        })
    };
    // a prepopulate keystore with hardcoded agents in it
    curve Predictable test_keystore();
);

// XXX: This may not be great to just grab an environment for this purpose.
//      It is assumed that this value is never really used in any "real"
//      way, because previously, it was implemented as a null pointer
//      wrapped in an UnsafeZomeCallWorkspace
fixturator!(
    HostFnWorkspace;
    curve Empty {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db();
        let keystore = holochain_keystore::test_keystore();
        tokio_helper::block_forever_on(async {
            fake_genesis(authored_db.to_db(), dht_db.to_db(), keystore.clone()).await.unwrap();
            HostFnWorkspace::new(
                authored_db.to_db(),
                dht_db.to_db(),
                cache.to_db(),
                keystore,
                Some(fixt!(AgentPubKey, Predictable, get_fixt_index!())),
            ).await.unwrap()
        })
    };
    curve Unpredictable {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db();
        let keystore = holochain_keystore::test_keystore();
        tokio_helper::block_forever_on(async {
            fake_genesis(authored_db.to_db(), dht_db.to_db(), keystore.clone()).await.unwrap();
            HostFnWorkspace::new(
                authored_db.to_db(),
                dht_db.to_db(),
                cache.to_db(),
                keystore,
                Some(fixt!(AgentPubKey, Predictable, get_fixt_index!())),
            ).await.unwrap()
        })
    };
    curve Predictable {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db_with_id(get_fixt_index!() as u8);
        let agent = fixt!(AgentPubKey, Predictable, get_fixt_index!());
        let keystore = holochain_keystore::test_keystore();
        tokio_helper::block_forever_on(async {
            crate::test_utils::fake_genesis_for_agent(authored_db.to_db(), dht_db.to_db(), agent.clone(), keystore.clone()).await.unwrap();
            HostFnWorkspace::new(
                authored_db.to_db(),
                dht_db.to_db(),
                cache.to_db(),
                keystore,
                Some(agent),
            ).await.unwrap()
        })
    };
);

fixturator!(
    HostFnWorkspaceRead;
    curve Empty {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db();
        let keystore = holochain_keystore::test_keystore();
        tokio_helper::block_forever_on(async {
            fake_genesis(authored_db.to_db(), dht_db.to_db(), keystore.clone()).await.unwrap();
            HostFnWorkspaceRead::new(
                authored_db.to_db().into(),
                dht_db.to_db().into(),
                cache.to_db(),
                keystore,
                Some(fixt!(AgentPubKey, Predictable, get_fixt_index!())),
            ).await.unwrap()
        })
    };
    curve Unpredictable {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db();
        let keystore = holochain_keystore::test_keystore();
        tokio_helper::block_forever_on(async {
            fake_genesis(authored_db.to_db(), dht_db.to_db(), keystore.clone()).await.unwrap();
            HostFnWorkspaceRead::new(
                authored_db.to_db().into(),
                dht_db.to_db().into(),
                cache.to_db(),
                keystore,
                Some(fixt!(AgentPubKey, Predictable, get_fixt_index!())),
            ).await.unwrap()
        })
    };
    curve Predictable {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db_with_id(get_fixt_index!() as u8);
        let agent = fixt!(AgentPubKey, Predictable, get_fixt_index!());
        let keystore = holochain_keystore::test_keystore();
        tokio_helper::block_forever_on(async {
            crate::test_utils::fake_genesis_for_agent(authored_db.to_db(), dht_db.to_db(), agent.clone(), keystore.clone()).await.unwrap();
            HostFnWorkspaceRead::new(
                authored_db.to_db().into(),
                dht_db.to_db().into(),
                cache.to_db(),
                keystore,
                Some(agent),
            ).await.unwrap()
        })
    };
);

fn make_call_zome_handle() -> CellConductorReadHandle {
    Arc::new(MockCellConductorReadHandleT::new())
}

fixturator!(
    CellConductorReadHandle;
    vanilla fn make_call_zome_handle();
);

fixturator!(
    ZomeCallHostAccess;
    curve Empty ZomeCallHostAccess {
        workspace: HostFnWorkspaceFixturator::new(Empty).next().unwrap(),
        keystore: MetaLairClientFixturator::new(Empty).next().unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        signal_tx: broadcast::channel(50).0,
        call_zome_handle: CellConductorReadHandleFixturator::new(Empty).next().unwrap(),
    };
    curve Unpredictable ZomeCallHostAccess {
        workspace: HostFnWorkspaceFixturator::new(Unpredictable).next().unwrap(),
        keystore: MetaLairClientFixturator::new(Unpredictable).next().unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        signal_tx: broadcast::channel(50).0,
        call_zome_handle: CellConductorReadHandleFixturator::new(Unpredictable).next().unwrap(),
    };
    curve Predictable ZomeCallHostAccess {
        workspace: HostFnWorkspaceFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        keystore: MetaLairClientFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        signal_tx: broadcast::channel(50).0,
        call_zome_handle: CellConductorReadHandleFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
    };
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
    curve Empty InitHostAccess {
        workspace: HostFnWorkspaceFixturator::new(Empty).next().unwrap(),
        keystore: MetaLairClientFixturator::new(Empty).next().unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        signal_tx: broadcast::channel(50).0,
        call_zome_handle: CellConductorReadHandleFixturator::new(Empty).next().unwrap(),
    };
    curve Unpredictable InitHostAccess {
        workspace: HostFnWorkspaceFixturator::new(Unpredictable).next().unwrap(),
        keystore: MetaLairClientFixturator::new(Unpredictable).next().unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        signal_tx: broadcast::channel(50).0,
        call_zome_handle: CellConductorReadHandleFixturator::new(Unpredictable).next().unwrap(),
    };
    curve Predictable InitHostAccess {
        workspace: HostFnWorkspaceFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        keystore: MetaLairClientFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        signal_tx: broadcast::channel(50).0,
        call_zome_handle: CellConductorReadHandleFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
    };
);

fixturator!(
    PostCommitInvocation;
    constructor fn new(CoordinatorZome, SignedActionHashedVec);
);

fixturator!(
    PostCommitHostAccess;
    curve Empty PostCommitHostAccess {
        workspace: HostFnWorkspaceFixturator::new(Empty).next().unwrap(),
        keystore: MetaLairClientFixturator::new(Empty).next().unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        signal_tx: broadcast::channel(50).0,
        call_zome_handle: Some(CellConductorReadHandleFixturator::new(Empty).next().unwrap()),
    };
    curve Unpredictable PostCommitHostAccess {
        workspace: HostFnWorkspaceFixturator::new(Unpredictable).next().unwrap(),
        keystore: MetaLairClientFixturator::new(Unpredictable).next().unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        signal_tx: broadcast::channel(50).0,
        call_zome_handle: Some(CellConductorReadHandleFixturator::new(Unpredictable).next().unwrap()),
    };
    curve Predictable PostCommitHostAccess {
        workspace: HostFnWorkspaceFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        keystore: MetaLairClientFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        signal_tx: broadcast::channel(50).0,
        call_zome_handle: Some(CellConductorReadHandleFixturator::new(Predictable).next().unwrap()),
    };
);

fixturator!(
    ZomesToInvoke;
    constructor fn one(Zome);
);

fixturator!(
    ValidateHostAccess;
    curve Empty ValidateHostAccess {
        workspace: HostFnWorkspaceReadFixturator::new(Empty).next().unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        is_inline: false,
    };
    curve Unpredictable ValidateHostAccess {
        workspace: HostFnWorkspaceReadFixturator::new(Unpredictable).next().unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        is_inline: false,
    };
    curve Predictable ValidateHostAccess {
        workspace: HostFnWorkspaceReadFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        network: Arc::new(MockHolochainP2pDnaT::new()),
        is_inline: false,
    };
);

fixturator!(
    HostContext;
    variants [
        ZomeCall(ZomeCallHostAccess)
        Validate(ValidateHostAccess)
        Init(InitHostAccess)
        EntryDefs(EntryDefsHostAccess)
        PostCommit(PostCommitHostAccess)
    ];
);

fixturator!(
    InvocationAuth;
    constructor fn new(AgentPubKey, CapSecret);
);

fixturator!(
    CallContext;
    constructor fn new(Zome, FunctionName, HostContext, InvocationAuth);
);
