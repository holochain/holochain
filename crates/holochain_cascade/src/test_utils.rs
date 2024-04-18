//! Test utils for holochain_cascade

use crate::authority;
use crate::authority::get_entry_ops_query::GetEntryOpsQuery;
use crate::authority::get_record_query::GetRecordOpsQuery;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::AnyDhtHashPrimitive;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holochain_nonce::Nonce256Bits;
use holochain_p2p::actor;
use holochain_p2p::dht_arc::DhtArc;
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::ChcImpl;
use holochain_p2p::HolochainP2pDnaT;
use holochain_p2p::HolochainP2pError;
use holochain_sqlite::rusqlite::Transaction;
use holochain_state::prelude::*;
use holochain_types::test_utils::chain::chain_to_ops;
use holochain_types::test_utils::chain::entry_hash;
use holochain_types::test_utils::chain::TestChainItem;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::dependencies::kitsune_p2p_fetch::OpHashSized;
use std::collections::HashSet;
use std::sync::Arc;
use QueryFilter;
use Signature;
use ValidationStatus;

pub use activity_test_data::*;
pub use entry_test_data::*;
use holochain_types::validation_receipt::ValidationReceiptBundle;
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
    pub fn authority_for_all(envs: Vec<DbRead<DbKindDht>>) -> Arc<Self> {
        Arc::new(Self {
            envs,
            authority: true,
        })
    }

    /// Declare that this node has zero coverage
    pub fn authority_for_nothing(envs: Vec<DbRead<DbKindDht>>) -> Arc<Self> {
        Arc::new(Self {
            envs,
            authority: false,
        })
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
        match dht_hash.into_primitive() {
            AnyDhtHashPrimitive::Entry(hash) => {
                for env in &self.envs {
                    let r =
                        authority::handle_get_entry(env.clone(), hash.clone(), (&options).into())
                            .await
                            .map_err(|e| HolochainP2pError::Other(e.into()))?;
                    out.push(WireOps::Entry(r));
                }
            }
            AnyDhtHashPrimitive::Action(hash) => {
                for env in &self.envs {
                    let r =
                        authority::handle_get_record(env.clone(), hash.clone(), (&options).into())
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

    async fn count_links(
        &self,
        query: WireLinkQuery,
    ) -> actor::HolochainP2pResult<CountLinksResponse> {
        let mut out = HashSet::new();

        for env in &self.envs {
            let r = authority::handle_get_links_query(env.clone(), query.clone())
                .await
                .map_err(|e| HolochainP2pError::Other(e.into()))?;
            out.extend(r);
        }

        Ok(CountLinksResponse::new(
            out.into_iter()
                .map(|l| l.create_link_hash)
                .collect::<Vec<_>>(),
        ))
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
        filter: ChainFilter,
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
        _dht_hash: holo_hash::OpBasis,
    ) -> actor::HolochainP2pResult<bool> {
        Ok(self.authority)
    }

    fn dna_hash(&self) -> holo_hash::DnaHash {
        todo!()
    }

    async fn send_remote_signal(
        &self,
        _from_agent: AgentPubKey,
        _to_agent_list: Vec<(Signature, AgentPubKey)>,
        _zome_name: ZomeName,
        _fn_name: FunctionName,
        _cap: Option<CapSecret>,
        _payload: ExternIO,
        _nonce: Nonce256Bits,
        _expires_at: Timestamp,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn publish(
        &self,
        _request_validation_receipt: bool,
        _countersigning_session: bool,
        _basis_hash: holo_hash::OpBasis,
        _source: AgentPubKey,
        _op_hash_list: Vec<OpHashSized>,
        _timeout_ms: Option<u64>,
        _reflect_ops: Option<Vec<crate::DhtOp>>,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn publish_countersign(
        &self,
        _flag: bool,
        _basis_hash: holo_hash::OpBasis,
        _op: crate::DhtOp,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn send_validation_receipts(
        &self,
        _to_agent: AgentPubKey,
        _receipts: ValidationReceiptBundle,
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
        _maybe_agent_info: Option<AgentInfoSigned>,
        _initial_arq: Option<Arq>,
    ) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn leave(&self, _agent: AgentPubKey) -> actor::HolochainP2pResult<()> {
        todo!()
    }

    async fn call_remote(
        &self,
        _from_agent: AgentPubKey,
        _from_signature: Signature,
        _to_agent: AgentPubKey,
        _zome_name: ZomeName,
        _fn_name: FunctionName,
        _cap: Option<CapSecret>,
        _payload: ExternIO,
        _nonce: Nonce256Bits,
        _expires_at: Timestamp,
    ) -> actor::HolochainP2pResult<holochain_serialized_bytes::SerializedBytes> {
        todo!()
    }

    fn chc(&self) -> Option<ChcImpl> {
        None
    }
}

/// Insert ops directly into the database and mark integrated as valid
pub async fn fill_db<Db: DbKindT + DbKindOp>(env: &DbWrite<Db>, op: DhtOpHashed) {
    env.write_async(move |txn| -> DatabaseResult<()> {
        let hash = op.as_hash();
        insert_op(txn, &op).unwrap();
        set_validation_status(txn, hash, ValidationStatus::Valid).unwrap();
        set_when_integrated(txn, hash, Timestamp::now()).unwrap();
        Ok(())
    })
    .await
    .unwrap();
}

/// Insert ops directly into the database and mark integrated as rejected
pub async fn fill_db_rejected<Db: DbKindT + DbKindOp>(env: &DbWrite<Db>, op: DhtOpHashed) {
    env.write_async(move |txn| -> DatabaseResult<()> {
        let hash = op.as_hash();
        insert_op(txn, &op).unwrap();
        set_validation_status(txn, hash, ValidationStatus::Rejected).unwrap();
        set_when_integrated(txn, hash, Timestamp::now()).unwrap();
        Ok(())
    })
    .await
    .unwrap();
}

/// Insert ops directly into the database and mark valid and pending integration
pub async fn fill_db_pending<Db: DbKindT + DbKindOp>(env: &DbWrite<Db>, op: DhtOpHashed) {
    env.write_async(move |txn| -> DatabaseResult<()> {
        let hash = op.as_hash();
        insert_op(txn, &op).unwrap();
        set_validation_status(txn, hash, ValidationStatus::Valid).unwrap();
        Ok(())
    })
    .await
    .unwrap();
}

/// Insert ops into the authored database
pub async fn fill_db_as_author(env: &DbWrite<DbKindAuthored>, op: DhtOpHashed) {
    env.write_async(move |txn| -> DatabaseResult<()> {
        insert_op(txn, &op).unwrap();
        Ok(())
    })
    .await
    .unwrap();
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
    match hash.into_primitive() {
        AnyDhtHashPrimitive::Entry(hash) => {
            WireOps::Entry(handle_get_entry_txn(txn, hash, options))
        }
        AnyDhtHashPrimitive::Action(hash) => {
            WireOps::Record(handle_get_record_txn(txn, hash, options))
        }
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

    db.test_write(move |txn| {
        for data in &data {
            for op in data {
                let op_light = DhtOpLite::RegisterAgentActivity(
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
                set_validation_status(txn, &hash, ValidationStatus::Valid).unwrap();
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
