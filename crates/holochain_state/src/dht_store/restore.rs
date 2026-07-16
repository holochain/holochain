//! Writes a pre-verified chain of [`Record`]s directly into the store as authored state, bypassing
//! validation limbo.

use std::collections::HashSet;

use holo_hash::{AgentPubKey, HasHash};
use holochain_data::dht::InsertChainOp;
use holochain_data::kind::Dht;
use holochain_data::DbWrite;
use holochain_types::EntryHashed;
use holochain_zome_types::dht_v2::{from_legacy_signed_action, RecordValidity};
use holochain_zome_types::prelude::{EntryVisibility, Record};

use crate::mutations::StateMutationResult;

use super::op_production::{build_ops_from_actions, cap_grant_index_params, encoded_chain_op_size};
use super::DhtStore;

impl DhtStore<DbWrite<Dht>> {
    /// Writes `records` into the store as authored state, in one transaction.
    ///
    /// `records` must be ordered genesis-to-head, with each record's action hash and
    /// `prev_action` link already verified. Every action is inserted as
    /// [`RecordValidity::Accepted`], bypassing validation limbo, and a `ChainOpPublish` row is
    /// inserted for every op, marking it ready for publish.
    pub async fn write_restored_chain(
        &self,
        author: &AgentPubKey,
        records: Vec<Record>,
    ) -> StateMutationResult<()> {
        let mut actions = Vec::with_capacity(records.len());
        let mut entries = Vec::new();
        for record in records {
            let (signed_action, record_entry) = record.into_inner();
            if let Some(entry) = record_entry.into_option() {
                if let Some(entry_hash) = signed_action.action().entry_hash() {
                    entries.push(EntryHashed::with_pre_hashed(entry, entry_hash.clone()));
                }
            }
            actions.push(signed_action);
        }

        // Entries whose authoring action declares them private go to `PrivateEntry`; every other
        // entry goes to the public `Entry` table.
        let private_entry_hashes: HashSet<_> = actions
            .iter()
            .filter_map(|sah| {
                let action = sah.action();
                let visibility = action.entry_visibility()?;
                if *visibility == EntryVisibility::Private {
                    action.entry_hash().cloned()
                } else {
                    None
                }
            })
            .collect();

        let (actions, ops) = build_ops_from_actions(actions)?;

        let mut tx = self.db().begin().await?;

        for entry_hashed in &entries {
            let entry_hash = entry_hashed.as_hash();
            let entry = entry_hashed.as_content();
            if private_entry_hashes.contains(entry_hash) {
                tx.insert_private_entry(entry_hash, author, entry).await?;
            } else {
                tx.insert_entry(entry_hash, entry).await?;
            }
        }

        for sah in &actions {
            let new_sah = from_legacy_signed_action(sah);
            tx.insert_action(&new_sah, Some(RecordValidity::Accepted))
                .await?;

            super::action_indexes::insert_action_indexes(
                &mut tx,
                new_sah.as_hash(),
                &new_sah.hashed.content.data,
            )
            .await?;

            if let Some((cap_access, tag)) = cap_grant_index_params(sah, &entries) {
                tx.insert_cap_grant(new_sah.as_hash(), cap_access, tag.as_deref())
                    .await?;
            }
        }

        for (op, op_hash, _op_order, timestamp, _deps) in &ops {
            let Some(op_as_chain) = op.as_chain_op() else {
                continue;
            };
            let basis_hash = op_as_chain.dht_basis().clone();
            let storage_center_loc = basis_hash.get_loc();
            let serialized_size = encoded_chain_op_size(op_as_chain, &actions, &entries);

            tx.insert_chain_op(InsertChainOp {
                op_hash,
                action_hash: op_as_chain.action_hash(),
                op_type: i64::from(op_as_chain.get_type()),
                basis_hash: &basis_hash,
                storage_center_loc,
                validation_status: RecordValidity::Accepted,
                locally_validated: true,
                require_receipt: false,
                when_received: *timestamp,
                when_integrated: *timestamp,
                serialized_size,
            })
            .await?;

            tx.insert_chain_op_publish(op_hash, None, None, None)
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
        AppEntryBytes, AppEntryDef, CapAccess, Create, Dna, EntryType, GrantedFunctions,
        ZomeCallCapGrant,
    };
    use holochain_zome_types::prelude::*;
    use std::sync::Arc;

    fn dht_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    fn make_record(action: Action, entry: Option<Entry>) -> Record {
        let action_hashed = ActionHashed::from_content_sync(action);
        let signed = SignedActionHashed::with_presigned(action_hashed, fixt!(Signature));
        Record::new(signed, entry)
    }

    fn dna_record(agent: &AgentPubKey) -> Record {
        make_record(
            Action::Dna(Dna {
                author: agent.clone(),
                timestamp: Timestamp::from_micros(0),
                hash: fixt!(DnaHash),
            }),
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
            Action::Create(Create {
                author: agent.clone(),
                timestamp: Timestamp::from_micros(1000),
                action_seq: 1,
                prev_action,
                entry_type,
                entry_hash,
                weight: Default::default(),
            }),
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

        // A StoreRecord chain op was written directly as Accepted/integrated, not into limbo.
        let op_hash = {
            use holochain_types::dht_op::ChainOpUniqueForm;
            use holochain_zome_types::op::ChainOpType;
            let (_, op_hash) =
                ChainOpUniqueForm::op_hash(ChainOpType::StoreRecord, create_action).unwrap();
            op_hash
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
