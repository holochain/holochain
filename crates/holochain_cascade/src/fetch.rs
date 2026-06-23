//! The cascade's network-fetch and local-cache path: fetching records, links,
//! and agent activity from peer authorities, verifying and caching the results,
//! and timing each fetch via [`CascadeDurationGuard`].

use super::*;

impl CascadeImpl {
    #[allow(clippy::result_large_err)] // TODO - investigate this lint
    fn insert_rendered_op(txn: &mut Txn<DbKindCache>, op: &RenderedOp) -> CascadeResult<()> {
        let RenderedOp {
            op_light,
            op_hash,
            action,
            validation_status,
            ..
        } = op;
        let op_order = OpOrder::new(op_light.get_type(), action.action().timestamp());
        let timestamp = action.action().timestamp();
        insert_action(txn, action)?;
        insert_op_lite(
            txn,
            op_light,
            op_hash,
            &op_order,
            &timestamp,
            // Using 0 value because this is the cache database and we only need sizes for gossip
            // in the DHT database.
            0,
            todo_no_cache_transfer_data(),
        )?;
        if let Some(status) = validation_status {
            set_validation_status(txn, op_hash, *status)?;
        }
        // We set the integrated to for the cache so it can match the
        // same query as the vault. This can also be used for garbage collection.
        set_when_integrated(txn, op_hash, Timestamp::now())?;
        Ok(())
    }

    #[allow(clippy::result_large_err)] // TODO - investigate this lint
    fn insert_rendered_ops(txn: &mut Txn<DbKindCache>, ops: &RenderedOps) -> CascadeResult<()> {
        let RenderedOps {
            ops,
            entry,
            warrant,
        } = ops;

        if let Some(warrant) = warrant {
            let op = DhtOpHashed::from_content_sync(warrant.clone());
            insert_op_cache(txn, &op)?;
        }
        if let Some(entry) = entry {
            insert_entry(txn, entry.as_hash(), entry.as_content())?;
        }
        for op in ops {
            Self::insert_rendered_op(txn, op)?;
        }
        Ok(())
    }

    /// Insert a set of agent activity into the Cache.
    #[allow(clippy::result_large_err)] // TODO - investigate this lint
    fn insert_activity(
        txn: &mut Txn<DbKindCache>,
        ops: Vec<RegisterAgentActivity>,
    ) -> CascadeResult<()> {
        for op in ops {
            let RegisterAgentActivity {
                action:
                    SignedHashed {
                        hashed: HoloHashed { content, .. },
                        signature,
                    },
                ..
            } = op;
            let op =
                DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(signature, content));
            insert_op_cache(txn, &op)?;
            // We set the integrated to for the cache so it can match the
            // same query as the vault. This can also be used for garbage collection.
            set_when_integrated(txn, op.as_hash(), Timestamp::now())?;
        }
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn merge_ops_into_cache(&self, responses: Vec<WireOps>) -> CascadeResult<()> {
        let cache = some_or_return!(self.cache.as_ref());

        // Extract the warrants, then render outside the cache transaction so
        // the transform is not counted as transaction time and the rendered
        // data can be reused by the `DhtStore` cache call below.
        let mut rendered_all: Vec<RenderedOps> = Vec::with_capacity(responses.len());
        let mut response_warrants: Vec<SignedWarrant> = Vec::new();
        for response in responses {
            let warrants = response.warrants().to_vec();
            let rendered = response.render()?;
            // Anti-DoS: a peer must prove any rejected record it serves with a
            // paired warrant. Without proof we drop the whole response rather
            // than be forced into pointless validation work.
            if rejected_without_warrant(&rendered, &warrants) {
                tracing::warn!("Dropping get response with a rejected record but no warrant");
                continue;
            }
            response_warrants.extend(warrants);
            rendered_all.push(rendered);
        }

        let rendered_for_legacy = rendered_all.clone();
        cache
            .write_async(move |txn| {
                for ops in &rendered_for_legacy {
                    Self::insert_rendered_ops(txn, ops)?;
                }
                CascadeResult::Ok(())
            })
            .await?;

        // Only signature-verified ops are written to the `DhtStore`, which is
        // where every cascade read resolves. The cache write above is retained
        // only so tests using synthetic signatures still find their fetched
        // ops, and is removed once those tests use real signatures.
        let verified = verify_rendered_ops_batch(rendered_all).await;
        self.cache_rendered_ops(&verified).await?;
        self.cache_response_warrants(response_warrants).await;

        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn merge_link_ops_into_cache(&self, responses: Vec<WireLinkOps>) -> CascadeResult<()> {
        let cache = some_or_return!(self.cache.as_ref());

        let mut rendered_all: Vec<RenderedOps> = Vec::with_capacity(responses.len());
        let mut response_warrants: Vec<SignedWarrant> = Vec::new();
        for response in responses {
            let warrants = response.warrants.clone();
            let rendered = response.render()?;
            if rejected_without_warrant(&rendered, &warrants) {
                tracing::warn!("Dropping get-links response with a rejected record but no warrant");
                continue;
            }
            response_warrants.extend(warrants);
            rendered_all.push(rendered);
        }

        let rendered_for_legacy = rendered_all.clone();
        cache
            .write_async(move |txn| {
                for ops in &rendered_for_legacy {
                    Self::insert_rendered_ops(txn, ops)?;
                }
                CascadeResult::Ok(())
            })
            .await?;

        let verified = verify_rendered_ops_batch(rendered_all).await;
        self.cache_rendered_ops(&verified).await?;
        self.cache_response_warrants(response_warrants).await;

        Ok(())
    }

    /// Cache the warrants that accompanied a get response into the `DhtStore`.
    async fn cache_response_warrants(&self, warrants: Vec<SignedWarrant>) {
        if warrants.is_empty() {
            return;
        }
        let warrant_ops = warrants.into_iter().map(WarrantOp::from).collect();
        if let Err(err) = self
            .dht_store
            .stage_warrants_for_validation(warrant_ops)
            .await
        {
            tracing::warn!(
                ?err,
                "DhtStore: stage_warrants_for_validation failed for get response"
            );
        }
    }

    /// Write a batch of rendered ops to the `DhtStore`, the source every cascade
    /// read resolves against.
    ///
    /// The op write is propagated: because the cascade re-reads from the
    /// `DhtStore` after fetching (rather than returning the network payload
    /// directly), a swallowed write would silently turn a successful response
    /// into a missing or incomplete read. Warrant staging is best-effort —
    /// warrants validate independently in limbo — so it is only logged.
    async fn cache_rendered_ops(&self, rendered_all: &[RenderedOps]) -> CascadeResult<()> {
        for rendered_ops in rendered_all {
            self.dht_store.cache_chain_ops(rendered_ops).await?;

            if let Some(warrant) = rendered_ops.warrant.as_ref() {
                if let Err(err) = self
                    .dht_store
                    .stage_warrants_for_validation(vec![warrant.clone()])
                    .await
                {
                    tracing::warn!(?err, "DhtStore: stage_warrants_for_validation failed");
                }
            }
        }
        Ok(())
    }

    /// Add new activity to the Cache.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn add_activity_into_cache(
        &self,
        response: MustGetAgentActivityResponse,
    ) -> CascadeResult<()> {
        let Some(cache) = self.cache.clone() else {
            return Ok(());
        };

        // Commit the activity to the chain.
        if let MustGetAgentActivityResponse::Activity { activity, warrants } = response {
            // TODO: Avoid this clone by committing the ops as references to the db.
            cache
                .write_async({
                    let activity = activity.clone();
                    let warrants = warrants.clone();
                    move |txn| {
                        Self::insert_activity(txn, activity)?;
                        for warrant in warrants {
                            let op = DhtOpHashed::from_content_sync(warrant);
                            insert_op_cache(txn, &op)?;
                        }

                        CascadeResult::Ok(())
                    }
                })
                .await?;

            // Signature verification gates writes into the `DhtStore`.
            let (activity, warrants) = verify_activity_signatures(activity, warrants).await;

            let activity_rendered = RenderedOps {
                entry: None,
                ops: activity
                    .iter()
                    .map(|ra| {
                        RenderedOp::new(
                            ra.action.action().clone(),
                            ra.action.signature().clone(),
                            None,
                            ChainOpType::RegisterAgentActivity,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                warrant: None,
            };

            self.dht_store.cache_chain_ops(&activity_rendered).await?;

            if !warrants.is_empty() {
                if let Err(err) = self.dht_store.stage_warrants_for_validation(warrants).await {
                    tracing::warn!(?err, "DhtStore: stage_warrants_for_validation failed");
                }
            }
        }

        Ok(())
    }

    fn add_warrants_into_scratch(&self, warrants: impl IntoIterator<Item = WarrantOp>) {
        let Some(scratch) = self.scratch.clone() else {
            return;
        };

        if let Err(err) = scratch.apply(move |scratch| {
            for warrant in warrants {
                scratch.add_warrant(SignedWarrant::new(
                    warrant.data().clone(),
                    warrant.signature().clone(),
                ));
            }
        }) {
            tracing::warn!(
                ?err,
                "Failed to add warrants from network response to scratch"
            );
        }
    }

    fn record_fetch_error(&self, fetch_type: &'static str) {
        let mut attrs = vec![opentelemetry::KeyValue::new("fetch_type", fetch_type)];
        if let Some((zome, fn_name)) = &self.zome_call_origin {
            attrs.push(opentelemetry::KeyValue::new("zome", zome.to_string()));
            attrs.push(opentelemetry::KeyValue::new("fn", fn_name.to_string()));
        }
        cascade_fetch_error_metric().add(1, &attrs);
    }

    /// Start timing a cascade query; the returned guard records
    /// `hc.cascade.duration` when dropped. See [`CascadeDurationGuard`].
    pub(crate) fn time_cascade(&self) -> CascadeDurationGuard {
        CascadeDurationGuard {
            start: Instant::now(),
            zome_call_origin: self.zome_call_origin.clone(),
        }
    }

    /// Fetch a Record from the network, caching and returning the results
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn fetch_record(
        &self,
        hash: AnyDhtHash,
        options: NetworkRequestOptions,
    ) -> CascadeResult<()> {
        let network = some_or_return!(self.network.as_ref());
        let results = match network
            .get(hash, options, self.zome_call_origin.clone())
            .instrument(debug_span!("fetch_record::network_get"))
            .await
        {
            Ok(ops) => ops,
            Err(e @ HolochainP2pError::NoPeersForLocation(_, _)) => {
                tracing::info!(?e, "No peers to fetch record from");
                vec![]
            }
            Err(e) => {
                self.record_fetch_error("record");
                return Err(e.into());
            }
        };

        self.merge_ops_into_cache(results).await?;
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub(crate) async fn fetch_links(
        &self,
        link_key: WireLinkKey,
        options: GetLinksRequestOptions,
    ) -> CascadeResult<()> {
        let network = some_or_return!(self.network.as_ref());
        let results = match network
            .get_links(link_key.clone(), options, self.zome_call_origin.clone())
            .await
        {
            Ok(link_ops) => link_ops,
            Err(e @ HolochainP2pError::NoPeersForLocation(_, _)) => {
                tracing::debug!(?e, "No peers to fetch links from");
                vec![]
            }
            Err(e) => {
                self.record_fetch_error("links");
                return Err(e.into());
            }
        };

        self.merge_link_ops_into_cache(results).await?;
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub(crate) async fn fetch_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> CascadeResult<Vec<AgentActivityResponse>> {
        let network = some_or_return!(self.network.as_ref(), Vec::with_capacity(0));
        let results = match network
            .get_agent_activity(agent, query, options, self.zome_call_origin.clone())
            .await
        {
            Ok(response) => response,
            Err(e @ HolochainP2pError::NoPeersForLocation(_, _)) => {
                tracing::debug!(?e, "No peers to fetch agent activity from");
                vec![]
            }
            Err(e) => {
                self.record_fetch_error("agent_activity");
                return Err(e.into());
            }
        };
        Ok(results)
    }

    /// Fetch hash bounded agent activity from the network.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub(crate) async fn fetch_must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
        options: NetworkRequestOptions,
    ) -> CascadeResult<MustGetAgentActivityResponse> {
        let network = self
            .network
            .as_ref()
            .ok_or(CascadeError::NetworkNotInitialized)?;

        let responses = match network
            .must_get_agent_activity(author, filter, options, self.zome_call_origin.clone())
            .await
        {
            Ok(responses) => responses,
            Err(e @ HolochainP2pError::NoPeersForLocation(_, _)) => {
                tracing::debug!(?e, "No peers to fetch agent activity from");
                return Err(e.into());
            }
            Err(e) => {
                self.record_fetch_error("must_get_agent_activity");
                return Err(e.into());
            }
        };

        // The network calls multiple peers but currently only returns a single response to here,
        // the first one it considers to be "non-empty".
        match responses.first() {
            None => Err(HolochainP2pError::Other("Received no responses".into()).into()),
            Some(selected_response) => {
                self.add_activity_into_cache(selected_response.clone())
                    .await?;

                if let MustGetAgentActivityResponse::Activity { warrants, .. } = selected_response {
                    self.add_warrants_into_scratch(warrants.iter().cloned());
                }

                Ok(selected_response.clone())
            }
        }
    }
}
