//! Writes a pre-verified chain of [`Record`]s directly into the store as authored state.

use holo_hash::{AgentPubKey, EntryHash, HasHash};
use holochain_data::dht::InsertChainOp;
use holochain_data::kind::Dht;
use holochain_data::DbWrite;
use holochain_types::op::{produce_ops_from_record, HashedChainOp};
use holochain_zome_types::prelude::{EntryHashed, EntryVisibility, Record, RecordValidity};

use crate::mutations::{StateMutationError, StateMutationResult};
use crate::source_chain::{cap_grant_index_params, encoded_chain_op_size};

use super::DhtStore;

impl DhtStore<DbWrite<Dht>> {
    /// Writes `records` into the store as authored state, in one transaction.
    ///
    /// `records` must be ordered genesis-to-head, with each record's action hash and `prev_action`
    /// link already verified.
    ///
    /// # Returns
    ///
    /// - [`StateMutationError::MismatchedEntryHash`] if an entry's hash doesn't match the one
    ///   declared in its action.
    /// - [`StateMutationError::AuthorsMustMatch`] if an action's author doesn't match `author`.
    pub async fn write_restored_chain(
        &self,
        author: &AgentPubKey,
        records: Vec<Record>,
    ) -> StateMutationResult<()> {
        let ops: Vec<HashedChainOp> = records.iter().flat_map(produce_ops_from_record).collect();

        let mut actions = Vec::with_capacity(records.len());
        let mut entries = Vec::with_capacity(records.len());
        for record in records {
            let (signed_action, record_entry) = record.into_inner();
            if signed_action.action().author() != author {
                return Err(StateMutationError::AuthorsMustMatch);
            }
            if let Some(entry) = record_entry.into_option() {
                let action = signed_action.action();
                if let Some(entry_hash) = action.entry_hash() {
                    if EntryHash::with_data_sync(&entry) != *entry_hash {
                        return Err(StateMutationError::MismatchedEntryHash);
                    }
                    let visibility = action.entry_visibility().copied().unwrap_or_default();
                    entries.push((
                        EntryHashed::with_pre_hashed(entry, entry_hash.clone()),
                        visibility,
                    ));
                }
            }
            actions.push(signed_action);
        }

        let mut tx = self.db().begin().await?;
        for (entry_hashed, visibility) in &entries {
            let entry_hash = entry_hashed.as_hash();
            let entry = entry_hashed.as_content();
            if visibility == &EntryVisibility::Private {
                tx.insert_private_entry(entry_hash, author, entry).await?;
            } else {
                tx.insert_entry(entry_hash, entry).await?;
            }
        }

        // Visibility no longer required so strip it
        let entries: Vec<_> = entries.into_iter().map(|(entry, _)| entry).collect();
        for sah in &actions {
            tx.insert_action(sah, Some(RecordValidity::Accepted))
                .await?;

            super::action_indexes::insert_action_indexes(
                &mut tx,
                sah.as_hash(),
                &sah.hashed.content.data,
            )
            .await?;

            if let Some((cap_access, tag)) = cap_grant_index_params(sah, &entries) {
                tx.insert_cap_grant(sah.as_hash(), cap_access, tag.as_deref())
                    .await?;
            }
        }

        for op in &ops {
            let serialized_size = encoded_chain_op_size(op, &entries);

            tx.insert_chain_op(InsertChainOp {
                op_hash: &op.op_hash,
                action_hash: op.action_hash(),
                op_type: i64::from(op.op_type),
                basis_hash: &op.basis_hash,
                storage_center_loc: op.storage_center_loc,
                validation_status: RecordValidity::Accepted,
                locally_validated: true,
                require_receipt: false,
                when_received: op.action.action().timestamp(),
                when_integrated: op.action.action().timestamp(),
                serialized_size,
            })
            .await?;

            tx.insert_chain_op_publish(&op.op_hash, None, None, None)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holo_hash::fixt::{AgentPubKeyFixturator, DnaHashFixturator};
    use holo_hash::{ActionHash, DnaHash, EntryHash};
    use holochain_serialized_bytes::UnsafeBytes;
    use holochain_types::prelude::{
        AppEntryBytes, AppEntryDef, CapAccess, EntryType, GrantedFunctions, ZomeCallCapGrant,
    };
    use holochain_zome_types::prelude::*;
    use std::sync::Arc;

    fn dht_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    fn make_record(action: Action, entry: Option<Entry>) -> Record {
        let entry_visibility = action.entry_visibility().copied();
        let action_hashed = holo_hash::HoloHashed::from_content_sync(action);
        let signed = SignedActionHashed::with_presigned(action_hashed, fixt!(Signature));
        let record_entry = RecordEntry::new(entry_visibility.as_ref(), entry);
        Record::new(signed, record_entry)
    }

    fn dna_record(agent: &AgentPubKey) -> Record {
        make_record(
            Action {
                header: ActionHeader {
                    author: agent.clone(),
                    timestamp: Timestamp::from_micros(0),
                    action_seq: 0,
                    prev_action: None,
                },
                data: ActionData::Dna(DnaData {
                    dna_hash: fixt!(DnaHash),
                }),
            },
            None,
        )
    }

    fn create_record(
        agent: &AgentPubKey,
        prev_action: ActionHash,
        entry_type: EntryType,
        entry: Entry,
    ) -> Record {
        let entry_hash = EntryHash::with_data_sync(&entry);
        make_record(
            Action {
                header: ActionHeader {
                    author: agent.clone(),
                    timestamp: Timestamp::from_micros(1000),
                    action_seq: 1,
                    prev_action: Some(prev_action),
                },
                data: ActionData::Create(CreateData {
                    entry_type,
                    entry_hash,
                }),
            },
            Some(entry),
        )
    }

    fn app_entry(seed: u8) -> Entry {
        Entry::App(AppEntryBytes(
            holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
        ))
    }

    #[tokio::test]
    async fn writes_action_entry_and_op_rows_as_accepted() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let author = fixt!(AgentPubKey);

        let dna = dna_record(&author);
        let create = create_record(
            &author,
            dna.action_address().clone(),
            EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            app_entry(1),
        );
        let create_hash = create.action_address().clone();
        let create_action = create.action().clone();
        let entry_hash = create.action().entry_hash().unwrap().clone();

        store
            .write_restored_chain(&author, vec![dna, create])
            .await
            .unwrap();

        // Both actions are present.
        assert!(store
            .db()
            .as_ref()
            .get_action(create_hash.clone())
            .await
            .unwrap()
            .is_some());

        // The public entry landed in the public Entry table.
        let entry = store
            .db()
            .as_ref()
            .get_entry(entry_hash, None)
            .await
            .unwrap();
        assert!(
            entry.is_some(),
            "entry should be readable without an author"
        );

        // Both actions round-trip via the author index too.
        let by_author = store
            .db()
            .as_ref()
            .get_actions_by_author(author.clone())
            .await
            .unwrap();
        assert_eq!(by_author.len(), 2);

        // A CreateRecord chain op was written directly as Accepted/integrated, not into limbo.
        let op_hash = {
            use holochain_types::op::ChainOpUniqueForm;
            use holochain_zome_types::op::ChainOpType;
            ChainOpUniqueForm::op_hash(ChainOpType::CreateRecord, &create_action)
        };
        let row = store
            .db()
            .as_ref()
            .get_chain_op(op_hash.clone())
            .await
            .unwrap()
            .expect("chain op row should exist");
        assert_eq!(row.validation_status, i64::from(RecordValidity::Accepted));
        assert_eq!(row.locally_validated, 1);
        assert!(row.when_integrated > 0);

        let publish_row = store
            .db()
            .as_ref()
            .get_chain_op_publish(op_hash)
            .await
            .unwrap();
        assert!(
            publish_row.is_some(),
            "a ChainOpPublish row should exist for the restored op"
        );
    }

    #[tokio::test]
    async fn mismatched_entry_hash_is_rejected() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let author = fixt!(AgentPubKey);

        let dna = dna_record(&author);
        let declared_hash = EntryHash::with_data_sync(&app_entry(1));
        let create_action = Action {
            header: ActionHeader {
                author: author.clone(),
                timestamp: Timestamp::from_micros(1000),
                action_seq: 1,
                prev_action: Some(dna.action_address().clone()),
            },
            data: ActionData::Create(CreateData {
                entry_type: EntryType::App(AppEntryDef::new(
                    0.into(),
                    0.into(),
                    EntryVisibility::Public,
                )),
                entry_hash: declared_hash,
            }),
        };
        let action_hashed = holo_hash::HoloHashed::from_content_sync(create_action);
        let signed = SignedActionHashed::with_presigned(action_hashed, fixt!(Signature));
        let create = Record::new(signed, RecordEntry::Present(app_entry(2)));

        let result = store.write_restored_chain(&author, vec![dna, create]).await;
        assert!(matches!(
            result,
            Err(StateMutationError::MismatchedEntryHash)
        ));
    }

    #[tokio::test]
    async fn foreign_author_action_is_rejected() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let author = fixt!(AgentPubKey);
        let impostor = fixt!(AgentPubKey);

        let dna = dna_record(&author);
        let create = create_record(
            &impostor,
            dna.action_address().clone(),
            EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Private,
            )),
            app_entry(1),
        );

        let result = store.write_restored_chain(&author, vec![dna, create]).await;
        assert!(matches!(result, Err(StateMutationError::AuthorsMustMatch)));
    }

    #[tokio::test]
    async fn identical_entry_content_with_different_visibility_is_written_to_both_tables() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let author = fixt!(AgentPubKey);

        let dna = dna_record(&author);
        let entry = app_entry(3);
        let entry_hash = EntryHash::with_data_sync(&entry);

        let public_create = create_record(
            &author,
            dna.action_address().clone(),
            EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry.clone(),
        );
        let private_create = make_record(
            Action {
                header: ActionHeader {
                    author: author.clone(),
                    timestamp: Timestamp::from_micros(2000),
                    action_seq: 2,
                    prev_action: Some(public_create.action_address().clone()),
                },
                data: ActionData::Create(CreateData {
                    entry_type: EntryType::App(AppEntryDef::new(
                        0.into(),
                        0.into(),
                        EntryVisibility::Private,
                    )),
                    entry_hash: entry_hash.clone(),
                }),
            },
            Some(entry),
        );

        store
            .write_restored_chain(&author, vec![dna, public_create, private_create])
            .await
            .unwrap();

        // The public copy is visible without an author.
        assert!(store
            .db()
            .as_ref()
            .get_entry(entry_hash.clone(), None)
            .await
            .unwrap()
            .is_some());

        // The private copy is visible when read back as the owning author.
        assert!(store
            .db()
            .as_ref()
            .get_entry(entry_hash, Some(&author))
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn private_entry_is_written_to_the_private_table() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let author = fixt!(AgentPubKey);

        let dna = dna_record(&author);
        let create = create_record(
            &author,
            dna.action_address().clone(),
            EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Private,
            )),
            app_entry(2),
        );
        let entry_hash = create.action().entry_hash().unwrap().clone();

        store
            .write_restored_chain(&author, vec![dna, create])
            .await
            .unwrap();

        // Not visible without the author.
        assert!(store
            .db()
            .as_ref()
            .get_entry(entry_hash.clone(), None)
            .await
            .unwrap()
            .is_none());

        // Visible when read back as the owning author.
        assert!(store
            .db()
            .as_ref()
            .get_entry(entry_hash, Some(&author))
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn cap_grant_entry_gets_an_index_row() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let author = fixt!(AgentPubKey);

        let dna = dna_record(&author);
        let grant =
            ZomeCallCapGrant::new("tag".into(), CapAccess::Unrestricted, GrantedFunctions::All);
        let create = create_record(
            &author,
            dna.action_address().clone(),
            EntryType::CapGrant,
            Entry::CapGrant(grant),
        );

        store
            .write_restored_chain(&author, vec![dna, create])
            .await
            .unwrap();

        let rows = store
            .db()
            .as_ref()
            .get_cap_grants_by_access(author, 0)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tag.as_deref(), Some("tag"));
    }
}
