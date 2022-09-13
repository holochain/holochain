//! Test utils for holochain_cascade

use crate::authority;
use crate::authority::get_entry_ops_query::GetEntryOpsQuery;
use crate::authority::get_record_query::GetRecordOpsQuery;
use holo_hash::hash_type::AnyDht;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holochain_p2p::actor;
use holochain_p2p::dht_arc::DhtArc;
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::ChcImpl;
use holochain_p2p::HolochainP2pDnaT;
use holochain_p2p::HolochainP2pError;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::db::DbKindDht;
use holochain_sqlite::db::DbKindOp;
use holochain_sqlite::db::DbKindT;
use holochain_sqlite::db::WriteManager;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::rusqlite::Transaction;
use holochain_state::mutations::insert_op;
use holochain_state::mutations::set_validation_status;
use holochain_state::mutations::set_when_integrated;
use holochain_state::prelude::insert_action;
use holochain_state::prelude::insert_op_lite;
use holochain_state::prelude::test_in_mem_db;
use holochain_state::prelude::Query;
use holochain_state::prelude::Txn;
use holochain_state::scratch::SyncScratch;
use holochain_types::activity::AgentActivityResponse;
use holochain_types::chain::MustGetAgentActivityResponse;
use holochain_types::db::DbRead;
use holochain_types::db::DbWrite;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::DhtOpLight;
use holochain_types::dht_op::OpOrder;
use holochain_types::dht_op::UniqueForm;
use holochain_types::dht_op::WireOps;
use holochain_types::link::WireLinkKey;
use holochain_types::link::WireLinkOps;
use holochain_types::metadata::MetadataSet;
use holochain_types::prelude::WireEntryOps;
use holochain_types::record::WireRecordOps;
use holochain_types::test_utils::chain::*;
use holochain_zome_types::ActionRefMut;
use holochain_zome_types::QueryFilter;
use holochain_zome_types::Timestamp;
use holochain_zome_types::ValidationStatus;

pub use activity_test_data::*;
pub use entry_test_data::*;
pub use record_test_data::*;

mod activity_test_data;
mod entry_test_data;
mod record_test_data;

/// A network implementation which routes to the local databases,
/// and can declare itself an authority either for all ops, or for no ops.
#[derive(Clone)]
pub struct PassThroughNetwork {
    envs: Vec<DbRead<DbKindDht>>,
    authority: bool,
}

impl PassThroughNetwork {
    /// Declare that this node has full coverage
    pub fn authority_for_all(envs: Vec<DbRead<DbKindDht>>) -> Self {
        Self {
            envs,
            authority: true,
        }
    }

    /// Declare that this node has zero coverage
    pub fn authority_for_nothing(envs: Vec<DbRead<DbKindDht>>) -> Self {
        Self {
            envs,
            authority: false,
        }
    }
}

/// A mutex-guarded [`MockHolochainP2pDnaT`]
#[derive(Clone)]
pub struct MockNetwork(std::sync::Arc<tokio::sync::Mutex<MockHolochainP2pDnaT>>);

impl MockNetwork {
    /// Constructor
    pub fn new(mock: MockHolochainP2pDnaT) -> Self {
        Self(std::sync::Arc::new(tokio::sync::Mutex::new(mock)))
    }
}

#[async_trait::async_trait]
impl HolochainP2pDnaT for PassThroughNetwork {
    async fn get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> actor::HolochainP2pResult<Vec<WireOps>> {
        let mut out = Vec::new();
        match *dht_hash.hash_type() {
            AnyDht::Entry => {
                for env in &self.envs {
                    let r = authority::handle_get_entry(
                        env.clone(),
                        dht_hash.clone().into(),
                        (&options).into(),
                    )
                    .await
                    .map_err(|e| HolochainP2pError::Other(e.into()))?;
                    out.push(WireOps::Entry(r));
                }
            }
            AnyDht::Action => {
                for env in &self.envs {
                    let r = authority::handle_get_record(
                        env.clone(),
                        dht_hash.clone().into(),
                        (&options).into(),
                    )
                    .await
                    .map_err(|e| HolochainP2pError::Other(e.into()))?;
                    out.push(WireOps::Record(r));
                }
            }
        }
        Ok(out)
    }

    async fn get_meta(
        &self,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: actor::GetMetaOptions,
    ) -> actor::HolochainP2pResult<Vec<MetadataSet>> {
        todo!()
    }

    async fn get_links(
        &self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> actor::HolochainP2pResult<Vec<WireLinkOps>> {
        let mut out = Vec::new();
        for env in &self.envs {
            let r = authority::handle_get_links(env.clone(), link_key.clone(), (&options).into())
                .await
                .map_err(|e| HolochainP2pError::Other(e.into()))?;
            out.push(r);
        }
        Ok(out)
    }

    async fn get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: QueryFilter,
        options: actor::GetActivityOptions,
    ) -> actor::HolochainP2pResult<Vec<AgentActivityResponse<ActionHash>>> {
        let mut out = Vec::new();
        for env in &self.envs {
            let r = authority::handle_get_agent_activity(
                env.clone(),
                agent.clone(),
                query.clone(),
                (&options).into(),
            )
            .await
            .map_err(|e| HolochainP2pError::Other(e.into()))?;
            out.push(r);
        }
        Ok(out)
    }

    async fn must_get_agent_activity(
        &self,
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> actor::HolochainP2pResult<Vec<MustGetAgentActivityResponse>> {
        let mut out = Vec::new();
        for env in &self.envs {
            let r = authority::handle_must_get_agent_activity(
                env.clone(),
                agent.clone(),
                filter.clone(),
            )
            .await
            .map_err(|e| HolochainP2pError::Other(e.into()))?;
            out.push(r);
        }
        Ok(out)
    }

    async fn authority_for_hash(
        &self,
        _dht_hash: holo_hash::AnyDhtHash,
    ) -> actor::HolochainP2pResult<bool> {
        Ok(self.authority)
    }

    fn dna_hash(&self) -> holo_hash::DnaHash {
        todo!()
    }

    async fn remote_signal(
        &self,
        _from_agent: AgentPubKey,
        _to_agent_list: Vec<AgentPubKey>,
        _zome_name: holochain_zome_types::ZomeName,
        _fn_name: holochain_zome_types::FunctionName,
        _cap: Option<holochain_zome_types::CapSecret>,
        _payload: holochain_zome_types::ExternIO,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn publish(
        &self,
        _request_validation_receipt: bool,
        _countersigning_session: bool,
        _dht_hash: holo_hash::AnyDhtHash,
        _ops: Vec<holochain_types::dht_op::DhtOp>,
        _timeout_ms: Option<u64>,
    ) -> actor::HolochainP2pResult<usize> {
        todo!()
    }

    async fn send_validation_receipt(
        &self,
        _to_agent: AgentPubKey,
        _receipt: holochain_serialized_bytes::SerializedBytes,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn countersigning_session_negotiation(
        &self,
        _agents: Vec<AgentPubKey>,
        _message: CountersigningSessionNegotiationMessage,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn new_integrated_data(&self) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn join(
        &self,
        _agent: AgentPubKey,
        _initial_arc: Option<DhtArc>,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn leave(&self, _agent: AgentPubKey) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn call_remote(
        &self,
        _from_agent: AgentPubKey,
        _to_agent: AgentPubKey,
        _zome_name: holochain_zome_types::ZomeName,
        _fn_name: holochain_zome_types::FunctionName,
        _cap: Option<holochain_zome_types::CapSecret>,
        _payload: holochain_zome_types::ExternIO,
    ) -> actor::HolochainP2pResult<holochain_serialized_bytes::SerializedBytes> {
        todo!()
    }

    fn chc(&self) -> Option<ChcImpl> {
        unimplemented!()
    }
}

/// Insert ops directly into the database and mark integrated as valid
pub fn fill_db<Db: DbKindT + DbKindOp>(env: &DbWrite<Db>, op: DhtOpHashed) {
    env.conn()
        .unwrap()
        .with_commit_sync(|txn| {
            let hash = op.as_hash();
            insert_op(txn, &op).unwrap();
            set_validation_status(txn, hash, ValidationStatus::Valid).unwrap();
            set_when_integrated(txn, hash, Timestamp::now()).unwrap();
            DatabaseResult::Ok(())
        })
        .unwrap();
}

/// Insert ops directly into the database and mark integrated as rejected
pub fn fill_db_rejected<Db: DbKindT + DbKindOp>(env: &DbWrite<Db>, op: DhtOpHashed) {
    env.conn()
        .unwrap()
        .with_commit_sync(|txn| {
            let hash = op.as_hash();
            insert_op(txn, &op).unwrap();
            set_validation_status(txn, hash, ValidationStatus::Rejected).unwrap();
            set_when_integrated(txn, hash, Timestamp::now()).unwrap();
            DatabaseResult::Ok(())
        })
        .unwrap();
}

/// Insert ops directly into the database and mark valid and pending integration
pub fn fill_db_pending<Db: DbKindT + DbKindOp>(env: &DbWrite<Db>, op: DhtOpHashed) {
    env.conn()
        .unwrap()
        .with_commit_sync(|txn| {
            let hash = op.as_hash();
            insert_op(txn, &op).unwrap();
            set_validation_status(txn, hash, ValidationStatus::Valid).unwrap();
            DatabaseResult::Ok(())
        })
        .unwrap();
}

/// Insert ops into the authored database
pub fn fill_db_as_author(env: &DbWrite<DbKindAuthored>, op: DhtOpHashed) {
    env.conn()
        .unwrap()
        .with_commit_sync(|txn| {
            insert_op(txn, &op).unwrap();
            DatabaseResult::Ok(())
        })
        .unwrap();
}

#[async_trait::async_trait]
impl HolochainP2pDnaT for MockNetwork {
    async fn get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> actor::HolochainP2pResult<Vec<WireOps>> {
        self.0.lock().await.get(dht_hash, options).await
    }

    async fn get_meta(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> actor::HolochainP2pResult<Vec<MetadataSet>> {
        self.0.lock().await.get_meta(dht_hash, options).await
    }

    async fn get_links(
        &self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> actor::HolochainP2pResult<Vec<WireLinkOps>> {
        self.0.lock().await.get_links(link_key, options).await
    }

    async fn get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: QueryFilter,
        options: actor::GetActivityOptions,
    ) -> actor::HolochainP2pResult<Vec<AgentActivityResponse<ActionHash>>> {
        self.0
            .lock()
            .await
            .get_agent_activity(agent, query, options)
            .await
    }

    async fn must_get_agent_activity(
        &self,
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> actor::HolochainP2pResult<Vec<MustGetAgentActivityResponse>> {
        self.0
            .lock()
            .await
            .must_get_agent_activity(agent, filter)
            .await
    }

    async fn authority_for_hash(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
    ) -> actor::HolochainP2pResult<bool> {
        self.0.lock().await.authority_for_hash(dht_hash).await
    }

    fn dna_hash(&self) -> holo_hash::DnaHash {
        todo!()
    }

    async fn remote_signal(
        &self,
        _from_agent: AgentPubKey,
        _to_agent_list: Vec<AgentPubKey>,
        _zome_name: holochain_zome_types::ZomeName,
        _fn_name: holochain_zome_types::FunctionName,
        _cap: Option<holochain_zome_types::CapSecret>,
        _payload: holochain_zome_types::ExternIO,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn publish(
        &self,
        _request_validation_receipt: bool,
        _countersigning_session: bool,
        _dht_hash: holo_hash::AnyDhtHash,
        _ops: Vec<holochain_types::dht_op::DhtOp>,
        _timeout_ms: Option<u64>,
    ) -> actor::HolochainP2pResult<usize> {
        todo!()
    }

    async fn send_validation_receipt(
        &self,
        _to_agent: AgentPubKey,
        _receipt: holochain_serialized_bytes::SerializedBytes,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn countersigning_session_negotiation(
        &self,
        _agents: Vec<AgentPubKey>,
        _message: CountersigningSessionNegotiationMessage,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn new_integrated_data(&self) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn join(
        &self,
        _agent: AgentPubKey,
        _initial_arc: Option<DhtArc>,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn leave(&self, _agent: AgentPubKey) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn call_remote(
        &self,
        _from_agent: AgentPubKey,
        _to_agent: AgentPubKey,
        _zome_name: holochain_zome_types::ZomeName,
        _fn_name: holochain_zome_types::FunctionName,
        _cap: Option<holochain_zome_types::CapSecret>,
        _payload: holochain_zome_types::ExternIO,
    ) -> actor::HolochainP2pResult<holochain_serialized_bytes::SerializedBytes> {
        todo!()
    }

    fn chc(&self) -> Option<ChcImpl> {
        unimplemented!()
    }
}

/// Utility for network simulation response to get entry.
pub fn handle_get_entry_txn(
    txn: &Transaction<'_>,
    hash: EntryHash,
    _options: holochain_p2p::event::GetOptions,
) -> WireEntryOps {
    let query = GetEntryOpsQuery::new(hash);
    query.run(Txn::from(txn)).unwrap()
}

/// Utility for network simulation response to get record.
pub fn handle_get_record_txn(
    txn: &Transaction<'_>,
    hash: ActionHash,
    options: holochain_p2p::event::GetOptions,
) -> WireRecordOps {
    let query = GetRecordOpsQuery::new(hash, options);
    query.run(Txn::from(txn)).unwrap()
}

/// Utility for network simulation response to get.
pub fn handle_get_txn(
    txn: &Transaction<'_>,
    hash: AnyDhtHash,
    options: holochain_p2p::event::GetOptions,
) -> WireOps {
    match *hash.hash_type() {
        AnyDht::Entry => WireOps::Entry(handle_get_entry_txn(txn, hash.into(), options)),
        AnyDht::Action => WireOps::Record(handle_get_record_txn(txn, hash.into(), options)),
    }
}

/// Commit the chain to a test in-memory database, returning a handle to that DB
pub fn commit_chain<Kind: DbKindT>(
    db_kind: Kind,
    chain: Vec<(AgentPubKey, Vec<TestChainItem>)>,
) -> DbWrite<Kind> {
    let data: Vec<_> = chain
        .into_iter()
        .map(|(a, c)| {
            chain_to_ops(c)
                .into_iter()
                .map(|mut op| {
                    *op.action.hashed.content.author_mut() = a.clone();
                    op
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let db = test_in_mem_db(db_kind);

    db.test_commit(|txn| {
        for data in &data {
            for op in data {
                let op_light = DhtOpLight::RegisterAgentActivity(
                    op.action.action_address().clone(),
                    op.action
                        .hashed
                        .entry_hash()
                        .cloned()
                        .unwrap_or_else(|| entry_hash(&[0]))
                        .into(),
                );

                let timestamp = Timestamp::now();
                let (_, hash) =
                    UniqueForm::op_hash(op_light.get_type(), op.action.hashed.content.clone())
                        .unwrap();
                insert_action(txn, &op.action).unwrap();
                insert_op_lite(
                    txn,
                    &op_light,
                    &hash,
                    &OpOrder::new(op_light.get_type(), timestamp),
                    &timestamp,
                )
                .unwrap();
                set_validation_status(txn, &hash, holochain_zome_types::ValidationStatus::Valid)
                    .unwrap();
                set_when_integrated(txn, &hash, Timestamp::now()).unwrap();
            }
        }
    });
    db
}

/// Add the items to the provided scratch
pub fn commit_scratch(scratch: SyncScratch, chain: Vec<(AgentPubKey, Vec<TestChainItem>)>) {
    let data = chain.into_iter().map(|(a, c)| {
        chain_to_ops(c)
            .into_iter()
            .map(|mut op| {
                *op.action.hashed.content.author_mut() = a.clone();
                op
            })
            .collect::<Vec<_>>()
    });

    scratch
        .apply(|scratch| {
            for data in data {
                for op in data {
                    scratch.add_action(op.action, Default::default());
                }
            }
        })
        .unwrap();
}
