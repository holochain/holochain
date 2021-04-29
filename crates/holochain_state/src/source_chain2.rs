use holo_hash::AgentPubKey;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
use holochain_keystore::KeystoreSender;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::dht_op::produce_op_lights_from_elements;
use holochain_types::dht_op::produce_op_lights_from_iter;
use holochain_types::dht_op::DhtOpLight;
use holochain_types::dht_op::OpOrder;
use holochain_types::dht_op::UniqueForm;
use holochain_types::element::SignedHeaderHashedExt;
use holochain_types::env::EnvRead;
use holochain_types::timestamp;
use holochain_types::EntryHashed;
use holochain_zome_types::CapGrant;
use holochain_zome_types::CapSecret;
use holochain_zome_types::Element;
use holochain_zome_types::Entry;
use holochain_zome_types::GrantedFunction;
use holochain_zome_types::HeaderBuilder;
use holochain_zome_types::HeaderBuilderCommon;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::HeaderInner;
use holochain_zome_types::QueryFilter;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::SignedHeaderHashed;

use crate::prelude::*;
use crate::query::chain_head::ChainHeadQuery;
use crate::scratch::Scratch;
use crate::scratch::SyncScratch;

#[derive(Clone)]
pub struct SourceChain {
    scratch: SyncScratch,
    vault: EnvRead,
    author: Arc<AgentPubKey>,
    persisted_len: u32,
    persisted_head: HeaderHash,
    public_only: bool,
}

impl SourceChain {
    pub fn new(vault: EnvRead, author: AgentPubKey) -> SourceChainResult<Self> {
        let scratch = Scratch::new().into_sync();
        let author = Arc::new(author);
        let (persisted_head, persisted_len) = vault
            .conn()?
            .with_reader(|txn| chain_head_db(&txn, author.clone()))?;
        Ok(Self {
            scratch,
            vault,
            author,
            persisted_len,
            persisted_head,
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
    pub fn elements(&self) -> SourceChainResult<Vec<Element>> {
        Ok(self.scratch.apply(|scratch| scratch.elements().collect())?)
    }

    pub async fn put<H: HeaderInner, B: HeaderBuilder<H>>(
        &self,
        header_builder: B,
        maybe_entry: Option<Entry>,
    ) -> SourceChainResult<HeaderHash> {
        // Check scratch for newer head.
        let (prev_header, header_seq) = self.scratch.apply(|scratch| {
            let chain_head = chain_head_scratch(&(*scratch), self.author.as_ref());
            let (prev_header, chain_len) =
                chain_head.unwrap_or_else(|| (self.persisted_head.clone(), self.persisted_len));
            let header_seq = chain_len + 1;
            (prev_header, header_seq)
        })?;

        // Build the header.
        let common = HeaderBuilderCommon {
            author: (*self.author).clone(),
            timestamp: timestamp::now(),
            header_seq,
            prev_header,
        };
        let header = header_builder.build(common).into();
        let header = HeaderHashed::from_content_sync(header);
        let hash = header.as_hash().clone();

        // Sign the header.
        let header = SignedHeaderHashed::new(self.vault.keystore(), header).await?;
        let element = Element::new(header, maybe_entry);

        // Put into scratch.
        self.scratch
            .apply(|scratch| insert_element_scratch(scratch, element))?;
        Ok(hash)
    }

    pub fn has_initialized(&self) -> SourceChainResult<bool> {
        Ok(self.len()? > 3)
    }

    pub fn len(&self) -> SourceChainResult<u32> {
        Ok(self.scratch.apply(|scratch| {
            let scratch_max = chain_head_scratch(&(*scratch), self.author.as_ref()).map(|(_, s)| s);
            scratch_max
                .map(|s| std::cmp::max(s, self.persisted_len))
                .unwrap_or(self.persisted_len)
        })?)
    }
    pub fn valid_cap_grant(
        &self,
        check_function: &GrantedFunction,
        check_agent: &AgentPubKey,
        check_secret: Option<&CapSecret>,
    ) -> SourceChainResult<Option<CapGrant>> {
        todo!("Implement cap query")
    }

    /// Query Headers in the source chain.
    /// This returns a Vec rather than an iterator because it is intended to be
    /// used by the `query` host function, which crosses the wasm boundary
    // FIXME: This query needs to be tested.
    pub fn query(&self, query: &QueryFilter) -> SourceChainResult<Vec<Element>> {
        let (range_min, range_max) = match query.sequence_range.clone() {
            Some(range) => (Some(range.start), Some(range.end)),
            None => (None, None),
        };
        let mut elements = self.vault.conn()?.with_reader(|txn| {
            let mut sql = "
                SELECT 
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
                        ":author": self.author.as_ref(),
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
                            let entry = from_blob::<Entry>(row.get("entry_blob")?)?;
                            Some(entry)
                        } else {
                            None
                        };
                        StateQueryResult::Ok(Element::new(shh, entry))
                    },
                )?
                .collect::<StateQueryResult<Vec<_>>>();
            elements
        })?;
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

    pub fn flush(&self) -> SourceChainResult<()> {
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
                    // Put the header back by value.
                    h = Some(header);
                    // Collect the DhtOpLight, DhtOpHash and OpOrder.
                    ops.push((op, op_hash, op_order));
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

        // Write the entries, headers and ops to the database in one transaction.
        self.vault.conn()?.with_commit(|txn| {
            // As at check.
            let (new_persisted_head, _) = chain_head_db(&txn, self.author.clone())?;
            match headers.last().map(|shh| shh.header_address()) {
                Some(scratch_head) => {
                    if self.persisted_head != new_persisted_head {
                        return Err(SourceChainError::HeadMoved(
                            Some(self.persisted_head.clone()),
                            Some(new_persisted_head),
                        ));
                    }
                }
                // Nothing to write
                None => return Ok(()),
            }

            for entry in entries {
                insert_entry(txn, entry)?;
            }
            for header in headers {
                insert_header(txn, header)?;
            }
            for (op, op_hash, op_order) in ops {
                insert_op_lite(txn, op, op_hash, true, op_order)?;
            }
            SourceChainResult::Ok(())
        })?;
        Ok(())
    }
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
        header = Some(h);
        hashes.push((op_hash, op_order));
    }
    let shh = SignedHeaderHashed::with_presigned(
        HeaderHashed::with_pre_hashed(header.expect("This can't be empty"), hash),
        signature,
    );
    if let Some(entry) = entry {
        insert_entry(txn, EntryHashed::from_content_sync(entry))?;
    }
    insert_header(txn, shh)?;
    for (op, (op_hash, op_order)) in ops.into_iter().zip(hashes) {
        insert_op_lite(txn, op, op_hash, true, op_order)?;
    }
    Ok(())
}

fn chain_head_db(
    txn: &Transaction,
    author: Arc<AgentPubKey>,
) -> SourceChainResult<(HeaderHash, u32)> {
    let chain_head = ChainHeadQuery::new(author);
    let (prev_header, last_header_seq) = chain_head
        .run(Txn::from(txn))?
        .ok_or(SourceChainError::ChainEmpty)?;
    Ok((prev_header, last_header_seq))
}

fn chain_head_scratch(scratch: &Scratch, author: &AgentPubKey) -> Option<(HeaderHash, u32)> {
    scratch
        .headers()
        .filter_map(|shh| {
            if shh.header().author() == author {
                Some((shh.header_address().clone(), shh.header().header_seq()))
            } else {
                None
            }
        })
        .max_by_key(|h| h.1)
}

async fn put_db<H: HeaderInner, B: HeaderBuilder<H>>(
    txn: &mut Transaction<'_>,
    keystore: &KeystoreSender,
    author: Arc<AgentPubKey>,
    header_builder: B,
    maybe_entry: Option<Entry>,
) -> SourceChainResult<HeaderHash> {
    let (prev_header, last_header_seq) = chain_head_db(txn, author.clone())?;
    let header_seq = last_header_seq + 1;

    let common = HeaderBuilderCommon {
        author: (*author).clone(),
        timestamp: timestamp::now(),
        header_seq,
        prev_header,
    };
    let header = header_builder.build(common).into();
    let header = HeaderHashed::from_content_sync(header);
    let header = SignedHeaderHashed::new(&keystore, header).await?;
    let element = Element::new(header, maybe_entry);
    let ops = produce_op_lights_from_elements(vec![&element])?;
    let (header, entry) = element.into_inner();
    let entry = entry.into_option();
    let hash = header.as_hash().clone();
    put_raw(txn, header, ops, entry)?;
    Ok(hash)
}
