pub mod curve;

use crate::conductor::api::CellConductorReadHandle;
use crate::conductor::api::MockCellConductorReadHandleT;
use crate::conductor::interface::SignalBroadcaster;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsHostAccess;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::init::InitHostAccess;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentHostAccess;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentInvocation;
use crate::core::ribosome::guest_callback::post_commit::PostCommitHostAccess;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateHostAccess;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageHostAccess;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomeCallHostAccess;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::sweettest::SweetDnaFile;
use crate::test_utils::fake_genesis;
use ::fixt::prelude::*;
pub use holo_hash::fixt::*;
use holo_hash::WasmHash;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDnaFixturator;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_state::test_utils::test_keystore;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use rand::seq::IteratorRandom;
use rand::thread_rng;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::Arc;
use strum::IntoEnumIterator;

pub use holochain_types::fixt::*;

newtype_fixturator!(FnComponents<Vec<String>>);

fixturator!(
    RealRibosome;
    constructor fn empty(DnaFile);
);

impl Iterator for RealRibosomeFixturator<curve::Zomes> {
    type Item = RealRibosome;

    fn next(&mut self) -> Option<Self::Item> {
        let input = self.0.curve.0.clone();
        let uuid = StringFixturator::new(Unpredictable).next().unwrap();
        let (dna_file, _, _) = tokio_helper::block_forever_on(async move {
            SweetDnaFile::from_test_wasms(uuid, input, Default::default()).await
        });

        let ribosome = RealRibosome::new(dna_file).unwrap();

        // warm the module cache for each wasm in the ribosome
        for zome in self.0.curve.0.clone() {
            let mut call_context = CallContextFixturator::new(Empty).next().unwrap();
            call_context.zome = CoordinatorZome::from(zome).erase_type();
            ribosome.runtime_compiled_module(call_context.zome.zome_name()).unwrap();
        }

        self.0.index += 1;

        Some(ribosome)
    }
}

fixturator!(
    DnaWasm;
    // note that an empty wasm will not compile
    curve Empty DnaWasm { code: Default::default() };
    curve Unpredictable TestWasm::iter().choose(&mut thread_rng()).unwrap().into();
    curve Predictable TestWasm::iter().cycle().nth(get_fixt_index!()).unwrap().into();
);

fixturator!(
    WasmMap;
    curve Empty BTreeMap::new().into();
    curve Unpredictable {
        let mut rng = rand::thread_rng();
        let number_of_wasms = rng.gen_range(0..5);

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

// fixturator!(
//     LinkMetaVal;
//     constructor fn new(ActionHash, EntryHash, Timestamp, u8, LinkTag);
// );

// impl Iterator for LinkMetaValFixturator<(EntryHash, LinkTag)> {
//     type Item = LinkMetaVal;
//     fn next(&mut self) -> Option<Self::Item> {
//         let mut f = fixt!(LinkMetaVal);
//         f.target = self.0.curve.0.clone();
//         f.tag = self.0.curve.1.clone();
//         Some(f)
//     }
// }

fixturator!(
    MetaLairClient;
    curve Empty {
        tokio_helper::block_forever_on(async {
            // an empty keystore
            holochain_keystore::test_keystore::spawn_test_keystore().await.unwrap()
        })
    };
    curve Unpredictable {
        // TODO: Make this unpredictable
        tokio_helper::block_forever_on(async {
            holochain_keystore::test_keystore::spawn_test_keystore().await.unwrap()
        })
    };
    // a prepopulate keystore with hardcoded agents in it
    curve Predictable test_keystore();
);

fixturator!(
    SignalBroadcaster;
    curve Empty {
        SignalBroadcaster::new(Vec::new())
    };
    curve Unpredictable {
        SignalBroadcaster::new(Vec::new())
    };
    curve Predictable {
        SignalBroadcaster::new(Vec::new())
    };
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
        let keystore = holochain_state::test_utils::test_keystore();
        tokio_helper::block_forever_on(async {
            fake_genesis(authored_db.to_db(), dht_db.to_db(), keystore.clone()).await.unwrap();
            HostFnWorkspace::new(
                authored_db.to_db(),
                dht_db.to_db(),
                DhtDbQueryCache::new(dht_db.to_db().into()),
                cache.to_db(),
                keystore,
                Some(fixt!(AgentPubKey, Predictable, get_fixt_index!())),
                Arc::new(fixt!(DnaDef))
            ).await.unwrap()
        })
    };
    curve Unpredictable {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db();
        let keystore = holochain_state::test_utils::test_keystore();
        tokio_helper::block_forever_on(async {
            fake_genesis(authored_db.to_db(), dht_db.to_db(), keystore.clone()).await.unwrap();
            HostFnWorkspace::new(
                authored_db.to_db(),
                dht_db.to_db(),
                DhtDbQueryCache::new(dht_db.to_db().into()),
                cache.to_db(),
                keystore,
                Some(fixt!(AgentPubKey, Predictable, get_fixt_index!())),
                Arc::new(fixt!(DnaDef))
            ).await.unwrap()
        })
    };
    curve Predictable {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db_with_id(get_fixt_index!() as u8);
        let agent = fixt!(AgentPubKey, Predictable, get_fixt_index!());
        let keystore = holochain_state::test_utils::test_keystore();
        tokio_helper::block_forever_on(async {
            crate::test_utils::fake_genesis_for_agent(authored_db.to_db(), dht_db.to_db(), agent.clone(), keystore.clone()).await.unwrap();
            HostFnWorkspace::new(
                authored_db.to_db(),
                dht_db.to_db(),
                DhtDbQueryCache::new(dht_db.to_db().into()),
                cache.to_db(),
                keystore,
                Some(agent),
                Arc::new(fixt!(DnaDef))
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
        let keystore = holochain_state::test_utils::test_keystore();
        tokio_helper::block_forever_on(async {
            fake_genesis(authored_db.to_db(), dht_db.to_db(), keystore.clone()).await.unwrap();
            HostFnWorkspaceRead::new(
                authored_db.to_db().into(),
                dht_db.to_db().into(),
                DhtDbQueryCache::new(dht_db.to_db().into()),
                cache.to_db(),
                keystore,
                Some(fixt!(AgentPubKey, Predictable, get_fixt_index!())),
                Arc::new(fixt!(DnaDef))
            ).await.unwrap()
        })
    };
    curve Unpredictable {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db();
        let keystore = holochain_state::test_utils::test_keystore();
        tokio_helper::block_forever_on(async {
            fake_genesis(authored_db.to_db(), dht_db.to_db(), keystore.clone()).await.unwrap();
            HostFnWorkspaceRead::new(
                authored_db.to_db().into(),
                dht_db.to_db().into(),
                DhtDbQueryCache::new(dht_db.to_db().into()),
                cache.to_db(),
                keystore,
                Some(fixt!(AgentPubKey, Predictable, get_fixt_index!())),
                Arc::new(fixt!(DnaDef)),
            ).await.unwrap()
        })
    };
    curve Predictable {
        let authored_db = holochain_state::test_utils::test_authored_db_with_id(get_fixt_index!() as u8);
        let dht_db = holochain_state::test_utils::test_dht_db_with_id(get_fixt_index!() as u8);
        let cache = holochain_state::test_utils::test_cache_db_with_id(get_fixt_index!() as u8);
        let agent = fixt!(AgentPubKey, Predictable, get_fixt_index!());
        let keystore = holochain_state::test_utils::test_keystore();
        tokio_helper::block_forever_on(async {
            crate::test_utils::fake_genesis_for_agent(authored_db.to_db(), dht_db.to_db(), agent.clone(), keystore.clone()).await.unwrap();
            HostFnWorkspaceRead::new(
                authored_db.to_db().into(),
                dht_db.to_db().into(),
                DhtDbQueryCache::new(dht_db.to_db().into()),
                cache.to_db(),
                keystore,
                Some(agent),
                Arc::new(fixt!(DnaDef))
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
    constructor fn new(HostFnWorkspace, MetaLairClient, HolochainP2pDna, SignalBroadcaster, CellConductorReadHandle);
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
    constructor fn new(HostFnWorkspace, MetaLairClient, HolochainP2pDna, SignalBroadcaster, CellConductorReadHandle);
);

fixturator!(
    MigrateAgentInvocation;
    constructor fn new(DnaDef, MigrateAgent);
);

fixturator!(
    MigrateAgentHostAccess;
    constructor fn new(HostFnWorkspace);
);

fixturator!(
    PostCommitInvocation;
    constructor fn new(CoordinatorZome, SignedActionHashedVec);
);

fixturator!(
    PostCommitHostAccess;
    constructor fn new(HostFnWorkspace, MetaLairClient, HolochainP2pDna);
);

fixturator!(
    ZomesToInvoke;
    constructor fn one(Zome);
);

fixturator!(
    ValidateHostAccess;
    constructor fn new(HostFnWorkspace, HolochainP2pDna);
);

fixturator!(
    ValidationPackageInvocation;
    constructor fn new(IntegrityZome, AppEntryDef);
);

fixturator!(
    ValidationPackageHostAccess;
    constructor fn new(HostFnWorkspace, HolochainP2pDna);
);

fixturator!(
    HostContext;
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
    InvocationAuth;
    constructor fn new(AgentPubKey, CapSecret);
);

fixturator!(
    CallContext;
    constructor fn new(Zome, FunctionName, HostContext, InvocationAuth);
);

fixturator!(
    ZomeCallInvocation;
    curve Empty ZomeCallInvocation {
        cell_id: CellIdFixturator::new(Empty).next().unwrap(),
        zome: ZomeFixturator::new(Empty).next().unwrap(),
        cap_secret: Some(CapSecretFixturator::new(Empty).next().unwrap()),
        fn_name: FunctionNameFixturator::new(Empty).next().unwrap(),
        payload: ExternIoFixturator::new(Empty).next().unwrap(),
        provenance: AgentPubKeyFixturator::new(Empty).next().unwrap(),
        signature: SignatureFixturator::new(Empty).next().unwrap(),
        nonce: Nonce256Bits::try_from(ThirtyTwoBytesFixturator::new(Empty).next().unwrap()).unwrap(),
        expires_at: TimestampFixturator::new(Empty).next().unwrap(),
    };
    curve Unpredictable ZomeCallInvocation {
        cell_id: CellIdFixturator::new(Unpredictable).next().unwrap(),
        zome: ZomeFixturator::new(Unpredictable).next().unwrap(),
        cap_secret: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
        fn_name: FunctionNameFixturator::new(Unpredictable).next().unwrap(),
        payload: ExternIoFixturator::new(Unpredictable).next().unwrap(),
        provenance: AgentPubKeyFixturator::new(Unpredictable).next().unwrap(),
        signature: SignatureFixturator::new(Unpredictable).next().unwrap(),
        nonce: Nonce256Bits::try_from(ThirtyTwoBytesFixturator::new(Unpredictable).next().unwrap()).unwrap(),
        // @todo should this be less predictable?
        expires_at: (Timestamp::now() + std::time::Duration::from_secs(10)).unwrap(),
    };
    curve Predictable ZomeCallInvocation {
        cell_id: CellIdFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        zome: ZomeFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        cap_secret: Some(CapSecretFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap()),
        fn_name: FunctionNameFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        payload: ExternIoFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        provenance: AgentPubKeyFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        signature: SignatureFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap(),
        nonce: Nonce256Bits::try_from(ThirtyTwoBytesFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap()).unwrap(),
        // @todo should this be more predictable?
        expires_at: (Timestamp::now() + std::time::Duration::from_secs(10)).unwrap(),
    };
);

/// Fixturator curve for a named zome invocation
/// cell id, test wasm for zome to call, function name, host input payload
pub struct NamedInvocation(pub CellId, pub TestWasm, pub String, pub ExternIO);

impl Iterator for ZomeCallInvocationFixturator<NamedInvocation> {
    type Item = ZomeCallInvocation;
    fn next(&mut self) -> Option<Self::Item> {
        let mut ret = ZomeCallInvocationFixturator::new(Unpredictable)
            .next()
            .unwrap();
        ret.cell_id = self.0.curve.0.clone();
        ret.zome = CoordinatorZome::from(self.0.curve.1).erase_type();
        ret.fn_name = self.0.curve.2.clone().into();
        ret.payload = self.0.curve.3.clone();

        // simulate a local transaction by setting the cap to empty and matching the provenance of
        // the call to the cell id
        ret.cap_secret = None;
        ret.provenance = ret.cell_id.agent_pubkey().clone();

        Some(ret)
    }
}
