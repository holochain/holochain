use std::collections::HashSet;

use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
use holochain_p2p::HolochainP2pCellT;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::dht_op::produce_op_lights_from_elements;
use holochain_types::dht_op::produce_op_lights_from_iter;
use holochain_types::dht_op::DhtOpLight;
use holochain_types::dht_op::DhtOpType;
use holochain_types::dht_op::OpOrder;
use holochain_types::dht_op::UniqueForm;
use holochain_types::element::SignedHeaderHashedExt;
use holochain_types::env::EnvRead;
use holochain_types::env::EnvWrite;
use holochain_types::timestamp;
use holochain_types::Timestamp;
use holochain_zome_types::entry::EntryHashed;
use holochain_zome_types::header;
use holochain_zome_types::CapAccess;
use holochain_zome_types::CapGrant;
use holochain_zome_types::CapSecret;
use holochain_zome_types::CounterSigningAgentState;
use holochain_zome_types::Element;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryVisibility;
use holochain_zome_types::GrantedFunction;
use holochain_zome_types::Header;
use holochain_zome_types::HeaderBuilder;
use holochain_zome_types::HeaderBuilderCommon;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::HeaderInner;
use holochain_zome_types::PreflightRequest;
use holochain_zome_types::QueryFilter;
use holochain_zome_types::Signature;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::SignedHeaderHashed;

use crate::chain_lock::is_chain_locked;
use crate::prelude::*;
use crate::query::chain_head::ChainHeadQuery;
use crate::scratch::Scratch;
use crate::scratch::SyncScratch;
use holochain_serialized_bytes::prelude::*;

pub use error::*;

mod error;
#[derive(Clone)]
pub struct SourceChain {
    scratch: SyncScratch,
    vault: EnvWrite,
    author: Arc<AgentPubKey>,
    persisted_seq: u32,
    persisted_head: HeaderHash,
    persisted_timestamp: Timestamp,
    public_only: bool,
}

// TODO fix this.  We shouldn't really have nil values but this would
// show if the database is corrupted and doesn't have an element
#[derive(Serialize, Deserialize)]
pub struct SourceChainJsonDump {
    pub elements: Vec<SourceChainJsonElement>,
    pub published_ops_count: usize,
}

#[derive(Serialize, Deserialize)]
pub struct SourceChainJsonElement {
    pub signature: Signature,
    pub header_address: HeaderHash,
    pub header: Header,
    pub entry: Option<Entry>,
}

// TODO: document that many functions here are only reading from the scratch,
//       not the entire source chain!
impl SourceChain {
    pub async fn new(vault: EnvWrite, author: AgentPubKey) -> SourceChainResult<Self> {
        let scratch = Scratch::new().into_sync();
        let author = Arc::new(author);
        let (persisted_head, persisted_seq, persisted_timestamp) = vault
            .async_reader({
                let author = author.clone();
                move |txn| chain_head_db(&txn, author)
            })
            .await?;
        Ok(Self {
            scratch,
            vault,
            author,
            persisted_seq,
            persisted_head,
            persisted_timestamp,
            public_only: false,
        })
    }
    pub fn public_only(&mut self) {
        self.public_only = true;
    }
    /// Take a snapshot of the scratch space that will
    /// not remain in sync with future updates.
    pub fn snapshot(&self) -> SourceChainResult<Scratch> {
        Ok(self.scratch.apply(|scratch| scratch.clone())?)
    }

    pub fn scratch(&self) -> SyncScratch {
        self.scratch.clone()
    }

    pub fn agent_pubkey(&self) -> &AgentPubKey {
        self.author.as_ref()
    }

    /// This has to clone all the data because we can't return
    /// references to constructed data.
    // TODO: Maybe we should store data as elements in the scratch?
    // TODO: document that this is only the elemnts in the SCRATCH, not the
    //       entire source chain!
    pub fn elements(&self) -> SourceChainResult<Vec<Element>> {
        Ok(self.scratch.apply(|scratch| scratch.elements().collect())?)
    }

    pub fn chain_head(&self) -> SourceChainResult<(HeaderHash, u32, Timestamp)> {
        // Check scratch for newer head.
        Ok(self.scratch.apply(|scratch| {
            let chain_head = chain_head_scratch(&(*scratch), self.author.as_ref());
            let (prev_header, header_seq, timestamp) = chain_head.unwrap_or_else(|| {
                (
                    self.persisted_head.clone(),
                    self.persisted_seq,
                    self.persisted_timestamp,
                )
            });
            (prev_header, header_seq, timestamp)
        })?)
    }

    async fn put_with_header(
        &self,
        header: Header,
        maybe_entry: Option<Entry>,
    ) -> SourceChainResult<HeaderHash> {
        let header = HeaderHashed::from_content_sync(header);
        let hash = header.as_hash().clone();
        let header = SignedHeaderHashed::new(&self.vault.keystore(), header).await?;
        let element = Element::new(header, maybe_entry);
        self.scratch
            .apply(|scratch| insert_element_scratch(scratch, element))?;
        Ok(hash)
    }

    pub async fn put_countersigned(&self, entry: Entry) -> SourceChainResult<HeaderHash> {
        if let Entry::CounterSign(ref session_data, _) = entry {
            self.put_with_header(
                Header::from_countersigning_data(session_data, (*self.author).clone())?,
                Some(entry),
            )
            .await
        } else {
            // The caller MUST guard against this case.
            unreachable!("Put countersigned called with the wrong entry type");
        }
    }

    pub async fn put<H: HeaderInner, B: HeaderBuilder<H>>(
        &self,
        header_builder: B,
        maybe_entry: Option<Entry>,
    ) -> SourceChainResult<HeaderHash> {
        let (prev_header, chain_head_seq, chain_head_timestamp) = self.chain_head()?;
        let header_seq = chain_head_seq + 1;

        // Build the header.
        let common = HeaderBuilderCommon {
            author: (*self.author).clone(),
            timestamp: std::cmp::max(timestamp::now(), chain_head_timestamp),
            header_seq,
            prev_header,
        };
        self.put_with_header(header_builder.build(common).into(), maybe_entry)
            .await
    }

    pub fn has_initialized(&self) -> SourceChainResult<bool> {
        Ok(self.len()? > 3)
    }

    pub fn is_empty(&self) -> SourceChainResult<bool> {
        Ok(self.len()? == 0)
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> SourceChainResult<u32> {
        Ok(self.scratch.apply(|scratch| {
            let scratch_max =
                chain_head_scratch(&(*scratch), self.author.as_ref()).map(|(_, s, _)| s);
            scratch_max
                .map(|s| std::cmp::max(s, self.persisted_seq))
                .unwrap_or(self.persisted_seq)
                + 1
        })?)
    }
    pub fn valid_cap_grant(
        &self,
        check_function: &GrantedFunction,
        check_agent: &AgentPubKey,
        check_secret: Option<&CapSecret>,
    ) -> SourceChainResult<Option<CapGrant>> {
        let author_grant = CapGrant::from(self.agent_pubkey().clone());
        if author_grant.is_valid(check_function, check_agent, check_secret) {
            return Ok(Some(author_grant));
        }
        // TODO: SQL_PERF: This query could have a fast upper bound if we add indexes.
        let valid_cap_grant = self.vault.conn()?.with_reader(|txn| {
            let not_referenced_header = "
            SELECT COUNT(H_REF.hash)
            FROM Header AS H_REF
            JOIN DhtOp AS D_REF ON D_REF.header_hash = H_REF.hash
            WHERE
            D_REF.is_authored = 1
            AND
            (
                H_REF.original_header_hash = Header.hash
                OR
                H_REF.deletes_header_hash = Header.hash
            )
            ";
            let sql = format!(
                "
                SELECT DISTINCT Entry.blob
                FROM Entry
                JOIN Header ON Header.entry_hash = Entry.hash
                JOIN DhtOp ON Header.hash = DhtOp.header_hash
                WHERE
                DhtOp.is_authored = 1
                AND
                Entry.access_type IS NOT NULL
                AND
                ({}) = 0
                ",
                not_referenced_header
            );
            txn.prepare(&sql)?
                .query_and_then([], |row| from_blob(row.get("blob")?))?
                .filter_map(|result: StateQueryResult<Entry>| match result {
                    Ok(entry) => entry
                        .as_cap_grant()
                        .filter(|grant| !matches!(grant, CapGrant::ChainAuthor(_)))
                        .filter(|grant| grant.is_valid(check_function, check_agent, check_secret))
                        .map(|cap| Some(Ok(cap)))
                        .unwrap_or(None),
                    Err(e) => Some(Err(e)),
                })
                // if there are still multiple grants, fold them down based on specificity
                // authorship > assigned > transferable > unrestricted
                .fold(
                    Ok(None),
                    |acc: StateQueryResult<Option<CapGrant>>, grant| {
                        let grant = grant?;
                        let acc = acc?;
                        let acc = match &grant {
                            CapGrant::RemoteAgent(zome_call_cap_grant) => {
                                match &zome_call_cap_grant.access {
                                    CapAccess::Assigned { .. } => match &acc {
                                        Some(CapGrant::RemoteAgent(acc_zome_call_cap_grant)) => {
                                            match acc_zome_call_cap_grant.access {
                                                // an assigned acc takes precedence
                                                CapAccess::Assigned { .. } => acc,
                                                // current grant takes precedence over all other accs
                                                _ => Some(grant),
                                            }
                                        }
                                        None => Some(grant),
                                        // authorship should be short circuit and filtered
                                        _ => unreachable!(),
                                    },
                                    CapAccess::Transferable { .. } => match &acc {
                                        Some(CapGrant::RemoteAgent(acc_zome_call_cap_grant)) => {
                                            match acc_zome_call_cap_grant.access {
                                                // an assigned acc takes precedence
                                                CapAccess::Assigned { .. } => acc,
                                                // transferable acc takes precedence
                                                CapAccess::Transferable { .. } => acc,
                                                // current grant takes preference over other accs
                                                _ => Some(grant),
                                            }
                                        }
                                        None => Some(grant),
                                        // authorship should be short circuited and filtered by now
                                        _ => unreachable!(),
                                    },
                                    CapAccess::Unrestricted => match acc {
                                        Some(_) => acc,
                                        None => Some(grant),
                                    },
                                }
                            }
                            // ChainAuthor should have short circuited and be filtered out already
                            _ => unreachable!(),
                        };
                        Ok(acc)
                    },
                )
        })?;
        Ok(valid_cap_grant)
    }

    /// Query Headers in the source chain.
    /// This returns a Vec rather than an iterator because it is intended to be
    /// used by the `query` host function, which crosses the wasm boundary
    // FIXME: This query needs to be tested.
    pub async fn query(&self, query: QueryFilter) -> SourceChainResult<Vec<Element>> {
        let (range_min, range_max) = match query.sequence_range.clone() {
            Some(range) => (Some(range.start), Some(range.end)),
            None => (None, None),
        };
        let author = self.author.clone();
        let mut elements = self
            .vault
            .async_reader({
                let query = query.clone();
                move |txn| {
                    let mut sql = "
                SELECT DISTINCT
                Header.hash AS header_hash, Header.blob AS header_blob
            "
                    .to_string();
                    if query.include_entries {
                        sql.push_str(
                            "
                    , Entry.blob AS entry_blob
                    ",
                        );
                    }
                    sql.push_str(
                        "
                FROM Header
                ",
                    );
                    if query.include_entries {
                        sql.push_str(
                            "
                    LEFT JOIN Entry On Header.entry_hash = Entry.hash
                    ",
                        );
                    }
                    sql.push_str(
                        "
                JOIN DhtOp On DhtOp.header_hash = Header.hash
                WHERE
                Header.author = :author
                AND
                DhtOp.is_authored = 1
                AND
                (:range_min IS NULL OR Header.seq >= :range_min)
                AND
                (:range_max IS NULL OR Header.seq < :range_max)
                AND
                (:entry_type IS NULL OR Header.entry_type = :entry_type)
                AND
                (:header_type IS NULL OR Header.type = :header_type)
                ",
                    );
                    let mut stmt = txn.prepare(&sql)?;
                    let elements = stmt
                        .query_and_then(
                            named_params! {
                                ":author": author.as_ref(),
                                ":range_min": range_min,
                                ":range_max": range_max,
                                ":entry_type": query.entry_type,
                                ":header_type": query.header_type,
                            },
                            |row| {
                                let header = from_blob::<SignedHeader>(row.get("header_blob")?)?;
                                let SignedHeader(header, signature) = header;
                                let hash: HeaderHash = row.get("header_hash")?;
                                let header = HeaderHashed::with_pre_hashed(header, hash);
                                let shh = SignedHeaderHashed::with_presigned(header, signature);
                                let entry = if query.include_entries {
                                    let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                                    match entry {
                                        Some(entry) => Some(from_blob::<Entry>(entry)?),
                                        None => None,
                                    }
                                } else {
                                    None
                                };
                                StateQueryResult::Ok(Element::new(shh, entry))
                            },
                        )?
                        .collect::<StateQueryResult<Vec<_>>>();
                    elements
                }
            })
            .await?;
        self.scratch.apply(|scratch| {
            let scratch_iter = scratch
                .headers()
                .filter(|shh| query.check(shh.header()))
                .filter_map(|shh| {
                    let entry = match shh.header().entry_hash() {
                        Some(eh) if query.include_entries => scratch.get_entry(eh).ok()?,
                        _ => None,
                    };
                    Some(Element::new(shh.clone(), entry))
                });
            elements.extend(scratch_iter);
        })?;
        Ok(elements)
    }

    pub async fn unlock_chain(&self) -> SourceChainResult<()> {
        self.vault
            .async_commit(move |txn| unlock_chain(txn))
            .await?;
        Ok(())
    }

    pub async fn accept_countersigning_preflight_request(
        &self,
        preflight_request: PreflightRequest,
        agent_index: u8,
    ) -> SourceChainResult<CounterSigningAgentState> {
        let hashed_preflight_request = holo_hash::encode::blake2b_256(
            &holochain_serialized_bytes::encode(&preflight_request)?,
        );

        // This all needs to be ensured in a non-panicky way BEFORE calling into the source chain here.
        let author = self.author.clone();
        assert_eq!(
            *author,
            preflight_request.signing_agents()[agent_index as usize].0
        );

        let countersigning_agent_state = self
            .vault
            .async_commit(move |txn| {
                if is_chain_locked(txn, &hashed_preflight_request)? {
                    return Err(SourceChainError::ChainLocked);
                }
                let (persisted_head, persisted_seq, _) = chain_head_db(&txn, author)?;
                let countersigning_agent_state =
                    CounterSigningAgentState::new(agent_index, persisted_head, persisted_seq);
                lock_chain(
                    txn,
                    &hashed_preflight_request,
                    preflight_request.session_times().end(),
                )?;
                SourceChainResult::Ok(countersigning_agent_state)
            })
            .await?;
        Ok(countersigning_agent_state)
    }

    pub async fn flush(
        &self,
        network: &(dyn HolochainP2pCellT + Send + Sync),
    ) -> SourceChainResult<()> {
        // Nothing to write
        if self.scratch.apply(|s| s.is_empty())? {
            return Ok(());
        }
        let (headers, ops, entries) = self.scratch.apply_and_then(|scratch| {
            let length = scratch.num_headers();

            // The op related data ends up here.
            let mut ops = Vec::with_capacity(length);

            // Drain out the headers.
            let signed_headers = scratch.drain_headers().collect::<Vec<_>>();
            // Headers end up back in here.
            let mut headers = Vec::with_capacity(signed_headers.len());

            // Loop through each header and produce op related data.
            for shh in signed_headers {
                // &HeaderHash, &Header, EntryHash are needed to produce the ops.
                let entry_hash = shh.header().entry_hash().cloned();
                let item = (shh.as_hash(), shh.header(), entry_hash);
                let ops_inner = produce_op_lights_from_iter(vec![item].into_iter(), 1)?;

                // Break apart the SignedHeaderHashed.
                let (header, sig) = shh.into_header_and_signature();
                let (header, hash) = header.into_inner();

                // We need to take the header by value and put it back each loop.
                let mut h = Some(header);
                for op in ops_inner {
                    let op_type = op.get_type();
                    // Header is required by value to produce the DhtOpHash.
                    let (header, op_hash) =
                        UniqueForm::op_hash(op_type, h.expect("This can't be empty"))?;
                    let op_order = OpOrder::new(op_type, header.timestamp());
                    let timestamp = header.timestamp();
                    let visibility = header.entry_type().map(|et| *et.visibility());
                    // Put the header back by value.
                    let dependency = get_dependency(op_type, &header);
                    h = Some(header);
                    // Collect the DhtOpLight, DhtOpHash and OpOrder.
                    ops.push((op, op_hash, op_order, timestamp, visibility, dependency));
                }

                // Put the SignedHeaderHashed back together.
                let shh = SignedHeaderHashed::with_presigned(
                    HeaderHashed::with_pre_hashed(h.expect("This can't be empty"), hash),
                    sig,
                );
                // Put the header back in the list.
                headers.push(shh);
            }

            // Drain out any entries.
            let entries = scratch.drain_entries().collect::<Vec<_>>();
            SourceChainResult::Ok((headers, ops, entries))
        })?;
        let mut ops_to_integrate = HashSet::with_capacity(ops.len());
        for op in &ops {
            if network.authority_for_hash(op.0.dht_basis().clone()).await? {
                ops_to_integrate.insert(op.1.clone());
            }
        }

        let maybe_countersigned_entry = entries
            .iter()
            .map(|entry| entry.as_content())
            .find(|entry| matches!(entry, Entry::CounterSign(_, _)));

        let lock = match maybe_countersigned_entry {
            Some(Entry::CounterSign(session_data, _)) => {
                if headers.len() != 1 {
                    return Err(SourceChainError::DirtyCounterSigningWrite);
                }
                holo_hash::encode::blake2b_256(&holochain_serialized_bytes::encode(
                    session_data.preflight_request(),
                )?)
            }
            _ => vec![],
        };

        // Write the entries, headers and ops to the database in one transaction.
        let author = self.author.clone();
        let persisted_head = self.persisted_head.clone();
        self.vault
            .async_commit(move |txn: &mut Transaction| {
                // As at check.
                let (new_persisted_head, _, _) = chain_head_db(&txn, author)?;
                if headers.last().is_none() {
                    // Nothing to write
                    return Ok(());
                }
                if persisted_head != new_persisted_head {
                    return Err(SourceChainError::HeadMoved(
                        Some(persisted_head),
                        Some(new_persisted_head),
                    ));
                }

                if is_chain_locked(txn, &lock)? {
                    return Err(SourceChainError::ChainLocked);
                }
                // If the lock is not just the empty lock, and the chain is NOT
                // locked then either the session expired or the countersigning
                // entry being committed now is the correct one for the lock,
                // in either case we should unlock the chain.
                else if !lock.is_empty() {
                    unlock_chain(txn)?;
                }

                for entry in entries {
                    insert_entry(txn, entry)?;
                }
                for header in headers {
                    insert_header(txn, header)?;
                }
                for (op, op_hash, op_order, timestamp, visibility, dependency) in ops {
                    let op_type = op.get_type();
                    insert_op_lite(txn, op, op_hash.clone(), true, op_order, timestamp)?;
                    set_validation_status(
                        txn,
                        op_hash.clone(),
                        holochain_zome_types::ValidationStatus::Valid,
                    )?;
                    set_dependency(txn, op_hash.clone(), dependency)?;
                    // TODO: Can anything every depend on a private store entry op? I don't think so.
                    let is_private_entry = op_type == DhtOpType::StoreEntry
                        && visibility == Some(EntryVisibility::Private);
                    if !is_private_entry && ops_to_integrate.contains(&op_hash) {
                        set_validation_stage(
                            txn,
                            op_hash,
                            ValidationLimboStatus::AwaitingIntegration,
                        )?;
                    }
                }
                SourceChainResult::Ok(())
            })
            .await?;
        Ok(())
    }
}

pub async fn genesis(
    vault: EnvWrite,
    dna_hash: DnaHash,
    agent_pubkey: AgentPubKey,
    membrane_proof: Option<SerializedBytes>,
) -> SourceChainResult<()> {
    let keystore = vault.keystore().clone();
    let dna_header = Header::Dna(header::Dna {
        author: agent_pubkey.clone(),
        timestamp: timestamp::now(),
        hash: dna_hash,
    });
    let dna_header = HeaderHashed::from_content_sync(dna_header);
    let dna_header = SignedHeaderHashed::new(&keystore, dna_header).await?;
    let dna_header_address = dna_header.as_hash().clone();
    let element = Element::new(dna_header, None);
    let dna_ops = produce_op_lights_from_elements(vec![&element])?;
    let (dna_header, _) = element.into_inner();

    // create the agent validation entry and add it directly to the store
    let agent_validation_header = Header::AgentValidationPkg(header::AgentValidationPkg {
        author: agent_pubkey.clone(),
        timestamp: timestamp::now(),
        header_seq: 1,
        prev_header: dna_header_address,
        membrane_proof,
    });
    let agent_validation_header = HeaderHashed::from_content_sync(agent_validation_header);
    let agent_validation_header =
        SignedHeaderHashed::new(&keystore, agent_validation_header).await?;
    let avh_addr = agent_validation_header.as_hash().clone();
    let element = Element::new(agent_validation_header, None);
    let avh_ops = produce_op_lights_from_elements(vec![&element])?;
    let (agent_validation_header, _) = element.into_inner();

    // create a agent chain element and add it directly to the store
    let agent_header = Header::Create(header::Create {
        author: agent_pubkey.clone(),
        timestamp: timestamp::now(),
        header_seq: 2,
        prev_header: avh_addr,
        entry_type: header::EntryType::AgentPubKey,
        entry_hash: agent_pubkey.clone().into(),
    });
    let agent_header = HeaderHashed::from_content_sync(agent_header);
    let agent_header = SignedHeaderHashed::new(&keystore, agent_header).await?;
    let element = Element::new(agent_header, Some(Entry::Agent(agent_pubkey)));
    let agent_ops = produce_op_lights_from_elements(vec![&element])?;
    let (agent_header, agent_entry) = element.into_inner();
    let agent_entry = agent_entry.into_option();

    vault
        .async_commit(move |txn| {
            source_chain::put_raw(txn, dna_header, dna_ops, None)?;
            source_chain::put_raw(txn, agent_validation_header, avh_ops, None)?;
            source_chain::put_raw(txn, agent_header, agent_ops, agent_entry)?;
            SourceChainResult::Ok(())
        })
        .await
}

pub fn put_raw(
    txn: &mut Transaction,
    shh: SignedHeaderHashed,
    ops: Vec<DhtOpLight>,
    entry: Option<Entry>,
) -> StateMutationResult<()> {
    let (header, signature) = shh.into_header_and_signature();
    let (header, hash) = header.into_inner();
    let mut header = Some(header);
    let mut hashes = Vec::with_capacity(ops.len());
    for op in &ops {
        let op_type = op.get_type();
        let (h, op_hash) =
            UniqueForm::op_hash(op_type, header.take().expect("This can't be empty"))?;
        let op_order = OpOrder::new(op_type, h.timestamp());
        let timestamp = h.timestamp();
        let visibility = h.entry_type().map(|et| *et.visibility());
        let dependency = get_dependency(op_type, &h);
        header = Some(h);
        hashes.push((
            op_hash, op_type, op_order, timestamp, visibility, dependency,
        ));
    }
    let shh = SignedHeaderHashed::with_presigned(
        HeaderHashed::with_pre_hashed(header.expect("This can't be empty"), hash),
        signature,
    );
    if let Some(entry) = entry {
        insert_entry(txn, EntryHashed::from_content_sync(entry))?;
    }
    insert_header(txn, shh)?;
    for (op, (op_hash, op_type, op_order, timestamp, visibility, dependency)) in
        ops.into_iter().zip(hashes)
    {
        insert_op_lite(txn, op, op_hash.clone(), true, op_order, timestamp)?;
        set_dependency(txn, op_hash.clone(), dependency)?;
        // TODO: SHARDING: Check if we are the authority here.
        // StoreEntry ops with private entries are never gossiped or published
        // so we don't need to integrate them.
        // TODO: Can anything every depend on a private store entry op? I don't think so.
        if !(op_type == DhtOpType::StoreEntry && visibility == Some(EntryVisibility::Private)) {
            set_validation_stage(txn, op_hash, ValidationLimboStatus::Pending)?;
        }
    }
    Ok(())
}

fn chain_head_db(
    txn: &Transaction,
    author: Arc<AgentPubKey>,
) -> SourceChainResult<(HeaderHash, u32, Timestamp)> {
    let chain_head = ChainHeadQuery::new(author);
    let (prev_header, last_header_seq, last_header_timestamp) = chain_head
        .run(Txn::from(txn))?
        .ok_or(SourceChainError::ChainEmpty)?;
    Ok((prev_header, last_header_seq, last_header_timestamp))
}

fn chain_head_scratch(
    scratch: &Scratch,
    author: &AgentPubKey,
) -> Option<(HeaderHash, u32, Timestamp)> {
    scratch
        .headers()
        .filter_map(|shh| {
            if shh.header().author() == author {
                Some((
                    shh.header_address().clone(),
                    shh.header().header_seq(),
                    shh.header().timestamp(),
                ))
            } else {
                None
            }
        })
        .max_by_key(|h| h.1)
}

#[cfg(test)]
async fn _put_db<H: HeaderInner, B: HeaderBuilder<H>>(
    vault: holochain_types::env::EnvWrite,
    author: Arc<AgentPubKey>,
    header_builder: B,
    maybe_entry: Option<Entry>,
) -> SourceChainResult<HeaderHash> {
    let (prev_header, last_header_seq, _) =
        fresh_reader!(vault, |txn| { chain_head_db(&txn, author.clone()) })?;
    let header_seq = last_header_seq + 1;

    let common = HeaderBuilderCommon {
        author: (*author).clone(),
        timestamp: timestamp::now(),
        header_seq,
        prev_header: prev_header.clone(),
    };
    let header = header_builder.build(common).into();
    let header = HeaderHashed::from_content_sync(header);
    let header = SignedHeaderHashed::new(&vault.keystore(), header).await?;
    let element = Element::new(header, maybe_entry);
    let ops = produce_op_lights_from_elements(vec![&element])?;
    let (header, entry) = element.into_inner();
    let entry = entry.into_option();
    let hash = header.as_hash().clone();
    vault.conn()?.with_commit_sync(|txn| {
        let (new_head, _, _) = chain_head_db(txn, author.clone())?;
        if new_head != prev_header {
            return Err(SourceChainError::HeadMoved(
                Some(prev_header),
                Some(new_head),
            ));
        }
        SourceChainResult::Ok(put_raw(txn, header, ops, entry)?)
    })?;
    Ok(hash)
}

/// dump the entire source chain as a pretty-printed json string
pub async fn dump_state(
    vault: EnvRead,
    author: AgentPubKey,
) -> Result<SourceChainJsonDump, SourceChainError> {
    Ok(vault
        .async_reader(move |txn| {
            let elements = txn
                .prepare(
                    "
                SELECT DISTINCT
                Header.blob AS header_blob, Entry.blob AS entry_blob,
                Header.hash AS header_hash
                FROM Header
                JOIN DhtOp ON DhtOp.header_hash = Header.hash
                LEFT JOIN Entry ON Header.entry_hash = Entry.hash
                WHERE
                DhtOp.is_authored = 1
                AND
                Header.author = :author
                ORDER BY Header.seq ASC
                ",
                )?
                .query_and_then(
                    named_params! {
                        ":author": author,
                    },
                    |row| {
                        let SignedHeader(header, signature) = from_blob(row.get("header_blob")?)?;
                        let header_address = row.get("header_hash")?;
                        let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                        let entry: Option<Entry> = match entry {
                            Some(entry) => Some(from_blob(entry)?),
                            None => None,
                        };
                        StateQueryResult::Ok(SourceChainJsonElement {
                            signature,
                            header_address,
                            header,
                            entry,
                        })
                    },
                )?
                .collect::<StateQueryResult<Vec<_>>>()?;
            let published_ops_count = txn.query_row(
                "
                SELECT COUNT(DhtOp.hash) FROM DhtOp
                JOIN Header ON DhtOp.header_hash = Header.hash
                WHERE
                DhtOp.is_authored = 1
                AND
                Header.author = :author
                AND
                last_publish_time IS NOT NULL
                ",
                named_params! {
                ":author": author,
                },
                |row| row.get(0),
            )?;
            StateQueryResult::Ok(SourceChainJsonDump {
                elements,
                published_ops_count,
            })
        })
        .await?)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_p2p::MockHolochainP2pCellT;
    use matches::assert_matches;

    use crate::source_chain::SourceChainResult;
    use holochain_zome_types::Entry;
    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_cap_grant() -> SourceChainResult<()> {
        let test_env = test_cell_env();
        let env = test_env.env();
        let secret = Some(CapSecretFixturator::new(Unpredictable).next().unwrap());
        let access = CapAccess::from(secret.unwrap());
        let mut mock = MockHolochainP2pCellT::new();
        mock.expect_authority_for_hash().returning(|_| Ok(false));

        // @todo curry
        let _curry = CurryPayloadsFixturator::new(Empty).next().unwrap();
        let function: GrantedFunction = ("foo".into(), "bar".into());
        let mut functions: GrantedFunctions = BTreeSet::new();
        functions.insert(function.clone());
        let grant = ZomeCallCapGrant::new("tag".into(), access.clone(), functions.clone());
        let mut agents = AgentPubKeyFixturator::new(Predictable);
        let alice = agents.next().unwrap();
        let bob = agents.next().unwrap();
        source_chain::genesis(env.clone(), fake_dna_hash(1), alice.clone(), None)
            .await
            .unwrap();

        {
            let chain = SourceChain::new(env.clone().into(), alice.clone()).await?;
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, secret.as_ref())?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );

            // bob should not match anything as the secret hasn't been committed yet
            assert_eq!(
                chain.valid_cap_grant(&function, &bob, secret.as_ref())?,
                None
            );
        }

        let (original_header_address, original_entry_address) = {
            let chain = SourceChain::new(env.clone().into(), alice.clone()).await?;
            let (entry, entry_hash) =
                EntryHashed::from_content_sync(Entry::CapGrant(grant.clone())).into_inner();
            let header_builder = builder::Create {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
            };
            let header = chain.put(header_builder, Some(entry)).await?;

            chain.flush(&mock).await.unwrap();

            (header, entry_hash)
        };

        {
            let chain = SourceChain::new(env.clone().into(), alice.clone()).await?;
            // alice should find her own authorship with higher priority than the committed grant
            // even if she passes in the secret
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, secret.as_ref())?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );

            // bob should be granted with the committed grant as it matches the secret he passes to
            // alice at runtime
            assert_eq!(
                chain.valid_cap_grant(&function, &bob, secret.as_ref())?,
                Some(grant.clone().into())
            );
        }

        // let's roll the secret and assign the grant to bob specifically
        let mut assignees = BTreeSet::new();
        assignees.insert(bob.clone());
        let updated_secret = Some(CapSecretFixturator::new(Unpredictable).next().unwrap());
        let updated_access = CapAccess::from((updated_secret.clone().unwrap(), assignees));
        let updated_grant = ZomeCallCapGrant::new("tag".into(), updated_access.clone(), functions);

        let (updated_header_hash, updated_entry_hash) = {
            let chain = SourceChain::new(env.clone().into(), alice.clone()).await?;
            let (entry, entry_hash) =
                EntryHashed::from_content_sync(Entry::CapGrant(updated_grant.clone())).into_inner();
            let header_builder = builder::Update {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
                original_header_address,
                original_entry_address,
            };
            let header = chain.put(header_builder, Some(entry)).await?;

            chain.flush(&mock).await.unwrap();

            (header, entry_hash)
        };

        {
            let chain = SourceChain::new(env.clone().into(), alice.clone()).await?;
            // alice should find her own authorship with higher priority than the committed grant
            // even if she passes in the secret
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, secret.as_ref())?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, updated_secret.as_ref())?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );

            // bob MUST provide the updated secret as the old one is invalidated by the new one
            assert_eq!(
                chain.valid_cap_grant(&function, &bob, secret.as_ref())?,
                None
            );
            assert_eq!(
                chain.valid_cap_grant(&function, &bob, updated_secret.as_ref())?,
                Some(updated_grant.into())
            );
        }

        {
            let chain = SourceChain::new(env.clone().into(), alice.clone()).await?;
            let header_builder = builder::Delete {
                deletes_address: updated_header_hash,
                deletes_entry_address: updated_entry_hash,
            };
            chain.put(header_builder, None).await?;

            chain.flush(&mock).await.unwrap();
        }

        {
            let chain = SourceChain::new(env.clone().into(), alice.clone()).await?;
            // alice should find her own authorship
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, secret.as_ref())?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, updated_secret.as_ref())?,
                Some(CapGrant::ChainAuthor(alice)),
            );

            // bob has no access
            assert_eq!(
                chain.valid_cap_grant(&function, &bob, secret.as_ref())?,
                None
            );
            assert_eq!(
                chain.valid_cap_grant(&function, &bob, updated_secret.as_ref())?,
                None
            );
        }

        Ok(())
    }

    // @todo bring all this back when we want to administer cap claims better
    // #[tokio::test(flavor = "multi_thread")]
    // async fn test_get_cap_claim() -> SourceChainResult<()> {
    //     let test_env = test_cell_env();
    //     let env = test_env.env();
    //     let env = env.conn().unwrap().await;
    //     let secret = CapSecretFixturator::new(Unpredictable).next().unwrap();
    //     let agent_pubkey = fake_agent_pubkey_1().into();
    //     let claim = CapClaim::new("tag".into(), agent_pubkey, secret.clone());
    //     {
    //         let mut store = SourceChainBuf::new(env.clone().into(), &env).await?;
    //         store
    //             .genesis(fake_dna_hash(1), fake_agent_pubkey_1(), None)
    //             .await?;
    //         arc.conn().unwrap().with_commit(|writer| store.flush_to_txn(writer))?;
    //     }
    //
    //     {
    //         let mut chain = SourceChain::new(env.clone().into(), &env).await?;
    //         chain.put_cap_claim(claim.clone()).await?;
    //
    // // ideally the following would work, but it won't because currently
    // // we can't get claims from the scratch space
    // // this will be fixed once we add the capability index
    //
    // // assert_eq!(
    // //     chain.get_persisted_cap_claim_by_secret(&secret)?,
    // //     Some(claim.clone())
    // // );
    //
    //         arc.conn().unwrap().with_commit(|writer| chain.flush_to_txn(writer))?;
    //     }
    //
    //     {
    //         let chain = SourceChain::new(env.clone().into(), &env).await?;
    //         assert_eq!(
    //             chain.get_persisted_cap_claim_by_secret(&secret).await?,
    //             Some(claim)
    //         );
    //     }
    //
    //     Ok(())

    #[tokio::test(flavor = "multi_thread")]
    async fn source_chain_buffer_iter_back() -> SourceChainResult<()> {
        observability::test_run().ok();
        let test_env = test_cell_env();
        let vault = test_env.env();
        let mut mock = MockHolochainP2pCellT::new();
        mock.expect_authority_for_hash().returning(|_| Ok(false));

        let author = test_env.cell_id().unwrap().agent_pubkey().clone();
        let author = Arc::new(author);

        fresh_reader_test!(vault, |txn| {
            assert_matches!(
                chain_head_db(&txn, author.clone()),
                Err(SourceChainError::ChainEmpty)
            );
        });
        genesis(
            vault.clone().into(),
            fixt!(DnaHash),
            (*author).clone(),
            None,
        )
        .await
        .unwrap();

        let source_chain = SourceChain::new(vault.clone().into(), (*author).clone())
            .await
            .unwrap();
        let entry = Entry::App(fixt!(AppEntryBytes));
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryType)),
            entry_hash: EntryHash::with_data_sync(&entry),
        };
        let h1 = source_chain.put(create, Some(entry)).await.unwrap();
        let entry = Entry::App(fixt!(AppEntryBytes));
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryType)),
            entry_hash: EntryHash::with_data_sync(&entry),
        };
        let h2 = source_chain.put(create, Some(entry)).await.unwrap();
        source_chain.flush(&mock).await.unwrap();

        fresh_reader_test!(vault, |txn| {
            assert_eq!(chain_head_db(&txn, author.clone()).unwrap().0, h2);
            // get the full element
            let store = Txn::from(&txn);
            let h1_element_fetched = store
                .get_element(&h1.clone().into())
                .expect("error retrieving")
                .expect("entry not found");
            let h2_element_fetched = store
                .get_element(&h2.clone().into())
                .expect("error retrieving")
                .expect("entry not found");
            assert_eq!(h1, *h1_element_fetched.header_address());
            assert_eq!(h2, *h2_element_fetched.header_address());
        });

        // check that you can iterate on the chain
        let source_chain = SourceChain::new(vault.clone().into(), (*author).clone())
            .await
            .unwrap();
        let res = source_chain.query(QueryFilter::new()).await.unwrap();
        assert_eq!(res.len(), 5);
        assert_eq!(*res[3].header_address(), h1);
        assert_eq!(*res[4].header_address(), h2);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn source_chain_buffer_dump_entries_json() -> SourceChainResult<()> {
        let test_env = test_cell_env();
        let vault = test_env.env();
        let author = test_env.cell_id().unwrap().agent_pubkey().clone();
        genesis(vault.clone().into(), fixt!(DnaHash), author.clone(), None)
            .await
            .unwrap();

        let json = dump_state(vault.clone().into(), author.clone()).await?;
        let json = serde_json::to_string_pretty(&json)?;
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["elements"][0]["header"]["type"], "Dna");
        assert_eq!(parsed["elements"][0]["entry"], serde_json::Value::Null);

        assert_eq!(parsed["elements"][2]["header"]["type"], "Create");
        assert_eq!(parsed["elements"][2]["header"]["entry_type"], "AgentPubKey");
        assert_eq!(parsed["elements"][2]["entry"]["entry_type"], "Agent");
        assert_ne!(
            parsed["elements"][2]["entry"]["entry"],
            serde_json::Value::Null
        );

        Ok(())
    }
}
