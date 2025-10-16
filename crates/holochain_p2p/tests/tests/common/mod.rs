use ::fixt::fixt;
use holo_hash::fixt::ActionHashFixturator;
use holochain_p2p::event::*;
use holochain_p2p::*;
use holochain_types::prelude::*;
use kitsune2_api::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Handler {
    pub calls: Arc<Mutex<Vec<String>>>,
    get_response: WireOps,
    get_response_delay: Option<Duration>,
}

impl Handler {
    pub(crate) fn new(get_response: WireOps, get_response_delay: Option<Duration>) -> Self {
        Self {
            calls: Default::default(),
            get_response,
            get_response_delay,
        }
    }
}

impl Default for Handler {
    fn default() -> Self {
        Handler {
            calls: Arc::new(Mutex::new(Vec::new())),
            get_response: WireOps::Entry(WireEntryOps::new()),
            get_response_delay: None,
        }
    }
}

impl HcP2pHandler for Handler {
    fn handle_call_remote(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        _signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        Box::pin(async move {
            let respond = format!(
                "got_call_remote: {}",
                String::from_utf8_lossy(&zome_call_params_serialized.0),
            );
            self.calls.lock().unwrap().push(respond.clone());
            Ok(UnsafeBytes::from(respond.into_bytes()).into())
        })
    }

    fn handle_publish(
        &self,
        _dna_hash: DnaHash,
        _ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push("publish".into());
            Ok(())
        })
    }

    fn handle_get(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _dht_hash: holo_hash::AnyDhtHash,
    ) -> BoxFut<'_, HolochainP2pResult<WireOps>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push("get".into());
            if let Some(delay) = self.get_response_delay {
                tokio::time::sleep(delay).await;
            }
            Ok(self.get_response.clone())
        })
    }

    fn handle_get_by_op_type(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _action_hash: ActionHash,
        _op_type: ChainOpType,
    ) -> BoxFut<'_, HolochainP2pResult<WireMaybeOpByType>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push("get_by_op_type".into());
            Ok(Some(WireOpByType(Judged {
                data: ChainOp::RegisterAgentActivity(fixt!(Signature), fixt!(Action)),
                status: Some(ValidationStatus::Valid),
            })))
        })
    }

    fn handle_get_meta(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<MetadataSet>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push("get_meta".into());
            Ok(MetadataSet {
                actions: Default::default(),
                invalid_actions: Default::default(),
                deletes: Default::default(),
                updates: Default::default(),
                entry_dht_status: Some(EntryDhtStatus::Live),
            })
        })
    }

    fn handle_get_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _link_key: WireLinkKey,
        _options: GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireLinkOps>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push("get_links".into());
            Ok(WireLinkOps {
                creates: vec![WireCreateLink::condense_base_only(
                    fixt!(CreateLink),
                    fixt!(Signature),
                    ValidationStatus::Valid,
                )],
                deletes: Vec::new(),
            })
        })
    }

    fn handle_count_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push("count_links".into());
            Ok(CountLinksResponse::new(vec![fixt!(ActionHash)]))
        })
    }

    fn handle_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _agent: AgentPubKey,
        _query: ChainQueryFilter,
        _options: GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<AgentActivityResponse>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push("get_agent_activity".into());
            Ok(AgentActivityResponse {
                agent: AgentPubKey::from_raw_36(vec![2; 36]),
                valid_activity: ChainItems::NotRequested,
                rejected_activity: ChainItems::NotRequested,
                status: ChainStatus::Valid(ChainHead {
                    action_seq: fixt!(Action).action_seq(),
                    hash: fixt!(ActionHash),
                }),
                highest_observed: None,
                warrants: Vec::new(),
            })
        })
    }

    fn handle_must_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _author: AgentPubKey,
        _filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<MustGetAgentActivityResponse>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap()
                .push("must_get_agent_activity".into());
            Ok(MustGetAgentActivityResponse::activity(Vec::new()))
        })
    }

    fn handle_validation_receipts_received(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap()
                .push("validation_receipts".into());
            Ok(())
        })
    }

    fn handle_publish_countersign(
        &self,
        _dna_hash: DnaHash,
        _op: ChainOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }

    fn handle_countersigning_session_negotiation(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _message: CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }
}
