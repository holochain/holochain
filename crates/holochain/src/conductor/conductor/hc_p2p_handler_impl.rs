use super::*;
use holochain_p2p::{HolochainP2pError, HolochainP2pResult};
use kitsune2_api::BoxFut;

impl Conductor {
    async fn cell_by_parts(
        &self,
        dna_hash: &DnaHash,
        agent: &AgentPubKey,
    ) -> HolochainP2pResult<Arc<Cell>> {
        let cell_id = CellId::new(dna_hash.clone(), agent.clone());
        self.cell_by_id(&cell_id)
            .await
            .map_err(HolochainP2pError::other)
    }
}

impl holochain_p2p::event::HcP2pHandler for Conductor {
    fn handle_call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        Box::pin(async {
            self.cell_by_parts(&dna_hash, &to_agent)
                .await?
                .handle_call_remote(dna_hash, to_agent, zome_call_params_serialized, signature)
                .await
        })
    }

    fn handle_publish(
        &self,
        dna_hash: DnaHash,
        request_validation_receipt: bool,
        countersigning_session: bool,
        ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            self.spaces
                .handle_publish(
                    &dna_hash,
                    request_validation_receipt,
                    countersigning_session,
                    ops,
                )
                .await
                .map_err(HolochainP2pError::other)
        })
    }

    fn handle_get(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: holochain_p2p::event::GetOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireOps>> {
        Box::pin(async {
            self.cell_by_parts(&dna_hash, &to_agent)
                .await?
                .handle_get(dna_hash, to_agent, dht_hash, options)
                .await
        })
    }

    fn handle_get_meta(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: holochain_p2p::event::GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<MetadataSet>> {
        Box::pin(async {
            self.cell_by_parts(&dna_hash, &to_agent)
                .await?
                .handle_get_meta(dna_hash, to_agent, dht_hash, options)
                .await
        })
    }

    fn handle_get_links(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: holochain_p2p::event::GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireLinkOps>> {
        Box::pin(async {
            self.cell_by_parts(&dna_hash, &to_agent)
                .await?
                .handle_get_links(dna_hash, to_agent, link_key, options)
                .await
        })
    }

    fn handle_count_links(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        Box::pin(async {
            self.cell_by_parts(&dna_hash, &to_agent)
                .await?
                .handle_count_links(dna_hash, to_agent, query)
                .await
        })
    }

    fn handle_get_agent_activity(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: holochain_p2p::event::GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<AgentActivityResponse>> {
        Box::pin(async {
            self.cell_by_parts(&dna_hash, &to_agent)
                .await?
                .handle_get_agent_activity(dna_hash, to_agent, agent, query, options)
                .await
        })
    }

    fn handle_must_get_agent_activity(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<MustGetAgentActivityResponse>> {
        Box::pin(async {
            self.cell_by_parts(&dna_hash, &to_agent)
                .await?
                .handle_must_get_agent_activity(dna_hash, to_agent, author, filter)
                .await
        })
    }

    fn handle_validation_receipts(
        &self,
        dna_hash: DnaHash,
        dht_op_list: Vec<DhtOpHash>,
    ) -> BoxFut<'_, HolochainP2pResult<ValidationReceiptBundle>> {
        Box::pin(async move {
            // get the list of signers for these ops
            let validators = self
                .running_cell_ids()
                .into_iter()
                .filter_map(|id| {
                    let (d, a) = id.into_dna_and_agent();
                    if d == dna_hash {
                        Some(a)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            let dht_db = self
                .spaces
                .dht_db(&dna_hash)
                .map_err(HolochainP2pError::other)?;

            // query the database
            let unsign: Vec<ValidationReceipt> = dht_db
                .read_async(move |txn| {
                    let mut stmt = txn.prepare(
                        "
                        SELECT
                            Action.author,
                            DhtOp.validation_status,
                            DhtOp.when_integrated
                        FROM DhtOp
                        JOIN Action ON DhtOp.action_hash = Action.hash
                        WHERE
                            DhtOp.hash = ?
                            AND DhtOp.when_integrated IS NOT NULL
                            AND DhtOp.validation_status IS NOT NULL
                        ",
                    )?;

                    let mut out = Vec::new();

                    // get the validation receipts if we are not the author
                    for dht_op_hash in dht_op_list {
                        for r in stmt.query_and_then([dht_op_hash.clone()], |r| {
                            let author: AgentPubKey = r.get("author")?;
                            let validation_status = r.get("validation_status")?;
                            let when_integrated = r.get("when_integrated")?;
                            StateQueryResult::Ok((
                                ValidationReceipt {
                                    dht_op_hash: dht_op_hash.clone(),
                                    validation_status,
                                    validators: validators.clone(),
                                    when_integrated,
                                },
                                author,
                            ))
                        })? {
                            if let Ok((receipt, author)) = r {
                                // Do NOT sign our own receipts
                                if validators.contains(&author) {
                                    continue;
                                }
                                out.push(receipt);
                            }
                        }
                    }

                    StateQueryResult::Ok(out)
                })
                .await
                .map_err(HolochainP2pError::other)?;

            let mut sign: Vec<SignedValidationReceipt> = Vec::new();

            // sign all the returned validation receipts
            for receipt in unsign {
                match ValidationReceipt::sign(receipt, self.keystore()).await {
                    Ok(Some(r)) => sign.push(r),
                    Ok(None) => (),
                    Err(err) => info!(failed_to_sign_receipt = ?err),
                }
            }

            // convert into the bundle type
            let out: ValidationReceiptBundle = sign.into();

            Ok(out)
        })
    }

    fn handle_countersigning_session_negotiation(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        message: holochain_p2p::event::CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async {
            self.cell_by_parts(&dna_hash, &to_agent)
                .await?
                .handle_countersigning_session_negotiation(dna_hash, to_agent, message)
                .await
        })
    }
}
