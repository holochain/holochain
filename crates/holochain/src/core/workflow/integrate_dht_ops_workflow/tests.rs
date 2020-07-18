#![cfg(test)]

use super::*;

use crate::core::ribosome::ZomeCallHostAccessFixturator;
use crate::here;
use crate::{
    core::{
        ribosome::{
            guest_callback::entry_defs::EntryDefsResult, host_fn, CallContextFixturator,
            MockRibosomeT,
        },
        state::{metadata::LinkMetaKey, workspace::WorkspaceError},
        workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace,
    },
    fixt::*,
};
use fixt::prelude::*;
use holo_hash::{Hashable, Hashed, HeaderHash};
use holo_hash_core::HoloHashCore;
use holochain_keystore::Signature;
use holochain_state::{
    env::{EnvironmentReadRef, EnvironmentWrite, EnvironmentWriteRef, ReadManager, WriteManager},
    error::DatabaseError,
    test_utils::test_cell_env,
};
use holochain_types::{
    composite_hash::{AnyDhtHash, EntryHash},
    dht_op::{DhtOp, DhtOpHashed},
    fixt::*,
    header::{builder, ElementDelete, EntryUpdate, LinkAdd, LinkRemove, NewEntryHeader},
    observability,
    validate::ValidationStatus,
    Entry, EntryHashed,
};
use holochain_zome_types::link::{LinkTag, Links};
use holochain_zome_types::{
    entry::GetOptions, entry_def::EntryDefs, zome::ZomeName, CommitEntryInput, GetEntryInput,
    GetLinksInput, LinkEntriesInput,
};
use produce_dht_ops_workflow::{produce_dht_ops_workflow, ProduceDhtOpsWorkspace};
use std::{collections::BTreeMap, convert::TryInto, sync::Arc};
use unwrap_to::unwrap_to;

#[derive(Clone)]
struct TestData {
    signature: Signature,
    original_entry: Entry,
    new_entry: Entry,
    any_header: Header,
    entry_update_header: EntryUpdate,
    entry_update_entry: EntryUpdate,
    original_header_hash: HeaderHash,
    original_entry_hash: EntryHash,
    new_entry_hash: EntryHash,
    original_header: NewEntryHeader,
    entry_delete: ElementDelete,
    link_add: LinkAdd,
    link_remove: LinkRemove,
}

impl TestData {
    async fn new() -> Self {
        // original entry
        let original_entry = fixt!(Entry);
        // New entry
        let new_entry = fixt!(Entry);
        Self::new_inner(original_entry, new_entry).await
    }

    #[instrument()]
    async fn new_inner(original_entry: Entry, new_entry: Entry) -> Self {
        // original entry
        let original_entry_hash = EntryHashed::with_data(original_entry.clone())
            .await
            .unwrap()
            .into_hash();

        // New entry
        let new_entry_hash = EntryHashed::with_data(new_entry.clone())
            .await
            .unwrap()
            .into_hash();

        // Original entry and header for updates
        let mut original_header = fixt!(NewEntryHeader);

        match &mut original_header {
            NewEntryHeader::Create(c) => c.entry_hash = original_entry_hash.clone(),
            NewEntryHeader::Update(u) => u.entry_hash = original_entry_hash.clone(),
        }

        let original_header_hash = HeaderHashed::with_data(original_header.clone().into())
            .await
            .unwrap()
            .into_hash();

        // Header for the new entry
        let mut new_entry_header = fixt!(NewEntryHeader);

        // Update to new entry
        match &mut new_entry_header {
            NewEntryHeader::Create(c) => c.entry_hash = new_entry_hash.clone(),
            NewEntryHeader::Update(u) => u.entry_hash = new_entry_hash.clone(),
        }

        // Entry update for header
        let mut entry_update_header = fixt!(EntryUpdate);
        entry_update_header.entry_hash = new_entry_hash.clone();
        entry_update_header.intended_for = IntendedFor::Header;
        entry_update_header.replaces_address = original_header_hash.clone();

        // Entry update for entry
        let mut entry_update_entry = fixt!(EntryUpdate);
        entry_update_entry.entry_hash = new_entry_hash.clone();
        entry_update_entry.intended_for = IntendedFor::Entry;
        entry_update_entry.replaces_address = original_header_hash.clone();

        // Entry delete
        let mut entry_delete = fixt!(ElementDelete);
        entry_delete.removes_address = original_header_hash.clone();

        // Link add
        let mut link_add = fixt!(LinkAdd);
        link_add.base_address = original_entry_hash.clone();
        link_add.target_address = new_entry_hash.clone();
        link_add.zome_id = fixt!(ZomeId);
        link_add.tag = fixt!(LinkTag);

        let link_add_hash = HeaderHashed::with_data(link_add.clone().into())
            .await
            .unwrap()
            .into_hash();

        // Link remove
        let mut link_remove = fixt!(LinkRemove);
        link_remove.base_address = original_entry_hash.clone();
        link_remove.link_add_address = link_add_hash.clone();

        Self {
            signature: fixt!(Signature),
            original_entry,
            new_entry,
            any_header: fixt!(Header),
            entry_update_header,
            entry_update_entry,
            original_header,
            original_header_hash,
            original_entry_hash,
            entry_delete,
            link_add,
            link_remove,
            new_entry_hash,
        }
    }

    /// Sets the Entries to App types
    async fn with_app_entry_type() -> Self {
        let original_entry = EntryFixturator::new(AppEntry).next().unwrap();
        let new_entry = EntryFixturator::new(AppEntry).next().unwrap();
        Self::new_inner(original_entry, new_entry).await
    }
}

#[derive(Clone)]
enum Db {
    Integrated(DhtOp),
    IntegratedEmpty,
    IntQueue(DhtOp),
    IntQueueEmpty,
    CasHeader(Header, Option<Signature>),
    CasEntry(Entry, Option<Header>, Option<Signature>),
    MetaEmpty,
    MetaHeader(Entry, Header),
    MetaActivity(Header),
    MetaUpdate(AnyDhtHash, Header),
    MetaDelete(EntryHash, HeaderHash, Header),
    MetaLink(LinkAdd, EntryHash),
    MetaLinkEmpty(LinkAdd),
}

impl Db {
    /// Checks that the database is in a state
    #[instrument(skip(expects, env_ref, dbs))]
    async fn check<'env>(
        expects: Vec<Self>,
        env_ref: &'env EnvironmentReadRef<'env>,
        dbs: &'env impl GetDb,
        here: String,
    ) {
        let reader = env_ref.reader().unwrap();
        let workspace = IntegrateDhtOpsWorkspace::new(&reader, dbs).unwrap();
        for expect in expects {
            match expect {
                Db::Integrated(op) => {
                    let op_hash = DhtOpHashed::with_data(op.clone()).await.into_hash();
                    let (op, basis) =
                        dht_op_to_light_basis(op, &workspace.cas)
                            .await
                            .expect(&format!(
                                "Failed to generate light {} for {}",
                                op_hash, here
                            ));
                    let value = IntegratedDhtOpsValue {
                        validation_status: ValidationStatus::Valid,
                        basis,
                        op,
                        when_integrated: Timestamp::now(),
                    };
                    let mut r = workspace.integrated_dht_ops.get(&op_hash).unwrap().unwrap();
                    r.when_integrated = value.when_integrated;
                    assert_eq!(r, value, "{}", here);
                }
                Db::IntQueue(op) => {
                    let value = IntegrationQueueValue {
                        validation_status: ValidationStatus::Valid,
                        op,
                    };
                    let res = workspace
                        .integration_queue
                        .iter()
                        .unwrap()
                        .filter_map(|(_, v)| if v == value { Ok(Some(v)) } else { Ok(None) })
                        .collect::<Vec<_>>()
                        .unwrap();
                    let exp = [value];
                    assert_eq!(&res[..], &exp[..], "{}", here,);
                }
                Db::CasHeader(header, _) => {
                    let hash = HeaderHashed::with_data(header.clone()).await.unwrap();
                    assert_eq!(
                        workspace
                            .cas
                            .get_header(hash.as_hash())
                            .await
                            .unwrap()
                            .expect(&format!("Header {:?} not in cas for {}", header, here))
                            .header(),
                        &header,
                        "{}",
                        here,
                    );
                }
                Db::CasEntry(entry, _, _) => {
                    let hash = EntryHashed::with_data(entry.clone())
                        .await
                        .unwrap()
                        .into_hash();
                    assert_eq!(
                        workspace
                            .cas
                            .get_entry(&hash)
                            .await
                            .unwrap()
                            .expect(&format!("Entry {:?} not in cas for {}", entry, here))
                            .into_content(),
                        entry,
                        "{}",
                        here,
                    );
                }
                Db::MetaHeader(entry, header) => {
                    let header_hash = HeaderHashed::with_data(header.clone())
                        .await
                        .unwrap()
                        .into_hash();
                    let entry_hash = EntryHashed::with_data(entry.clone())
                        .await
                        .unwrap()
                        .into_hash();
                    let res = workspace
                        .meta
                        .get_headers(entry_hash)
                        .unwrap()
                        .collect::<Vec<_>>()
                        .unwrap();
                    let exp = [header_hash];
                    assert_eq!(&res[..], &exp[..], "{}", here,);
                }
                Db::MetaActivity(header) => {
                    let header_hash = HeaderHashed::with_data(header.clone())
                        .await
                        .unwrap()
                        .into_hash();
                    let res = workspace
                        .meta
                        .get_activity(header.author().clone())
                        .unwrap()
                        .collect::<Vec<_>>()
                        .unwrap();
                    let exp = [header_hash];
                    assert_eq!(&res[..], &exp[..], "{}", here,);
                }
                Db::MetaUpdate(base, header) => {
                    let header_hash = HeaderHashed::with_data(header.clone())
                        .await
                        .unwrap()
                        .into_hash();
                    let res = workspace
                        .meta
                        .get_updates(base)
                        .unwrap()
                        .collect::<Vec<_>>()
                        .unwrap();
                    let exp = [header_hash];
                    assert_eq!(&res[..], &exp[..], "{}", here,);
                }
                Db::MetaDelete(base, deleted_header_hash, header) => {
                    let header_hash = HeaderHashed::with_data(header.clone())
                        .await
                        .unwrap()
                        .into_hash();
                    let res = workspace
                        .meta
                        .get_deletes_on_entry(base)
                        .unwrap()
                        .collect::<Vec<_>>()
                        .unwrap();
                    let res2 = workspace
                        .meta
                        .get_deletes_on_header(deleted_header_hash)
                        .unwrap()
                        .collect::<Vec<_>>()
                        .unwrap();
                    let exp = [header_hash];
                    assert_eq!(&res[..], &exp[..], "{}", here,);
                    assert_eq!(&res2[..], &exp[..], "{}", here,);
                }
                Db::IntegratedEmpty => {
                    assert_eq!(
                        workspace
                            .integrated_dht_ops
                            .iter()
                            .unwrap()
                            .count()
                            .unwrap(),
                        0,
                        "{}",
                        here
                    );
                }
                Db::IntQueueEmpty => {
                    assert_eq!(
                        workspace.integration_queue.iter().unwrap().count().unwrap(),
                        0,
                        "{}",
                        here
                    );
                }
                Db::MetaEmpty => {
                    // TODO: Not currently possible because kvv bufs have no iterator over all keys
                }
                Db::MetaLink(link_add, target_hash) => {
                    let link_add_hash = HeaderHashed::with_data(link_add.clone().into())
                        .await
                        .unwrap()
                        .into_hash();

                    // LinkMetaKey
                    let mut link_meta_keys = Vec::new();
                    link_meta_keys.push(LinkMetaKey::Full(
                        &link_add.base_address,
                        link_add.zome_id,
                        &link_add.tag,
                        &link_add_hash,
                    ));
                    link_meta_keys.push(LinkMetaKey::BaseZomeTag(
                        &link_add.base_address,
                        link_add.zome_id,
                        &link_add.tag,
                    ));
                    link_meta_keys.push(LinkMetaKey::BaseZome(
                        &link_add.base_address,
                        link_add.zome_id,
                    ));
                    link_meta_keys.push(LinkMetaKey::Base(&link_add.base_address));

                    for link_meta_key in link_meta_keys {
                        let res = workspace.meta.get_links(&link_meta_key).unwrap();

                        assert_eq!(res.len(), 1, "{}", here);
                        assert_eq!(res[0].link_add_hash, link_add_hash, "{}", here);
                        assert_eq!(res[0].target, target_hash, "{}", here);
                        assert_eq!(res[0].zome_id, link_add.zome_id, "{}", here);
                        assert_eq!(res[0].tag, link_add.tag, "{}", here);
                    }
                }
                Db::MetaLinkEmpty(link_add) => {
                    let link_add_hash = HeaderHashed::with_data(link_add.clone().into())
                        .await
                        .unwrap()
                        .into_hash();

                    // LinkMetaKey
                    let mut link_meta_keys = Vec::new();
                    link_meta_keys.push(LinkMetaKey::Full(
                        &link_add.base_address,
                        link_add.zome_id,
                        &link_add.tag,
                        &link_add_hash,
                    ));
                    link_meta_keys.push(LinkMetaKey::BaseZomeTag(
                        &link_add.base_address,
                        link_add.zome_id,
                        &link_add.tag,
                    ));
                    link_meta_keys.push(LinkMetaKey::BaseZome(
                        &link_add.base_address,
                        link_add.zome_id,
                    ));
                    link_meta_keys.push(LinkMetaKey::Base(&link_add.base_address));

                    for link_meta_key in link_meta_keys {
                        let res = workspace.meta.get_links(&link_meta_key).unwrap();

                        assert_eq!(res.len(), 0, "{}", here);
                    }
                }
            }
        }
    }

    // Sets the database to a certain state
    #[instrument(skip(pre_state, env_ref, dbs))]
    async fn set<'env>(
        pre_state: Vec<Self>,
        env_ref: &'env EnvironmentWriteRef<'env>,
        dbs: &impl GetDb,
    ) {
        let reader = env_ref.reader().unwrap();
        let mut workspace = IntegrateDhtOpsWorkspace::new(&reader, dbs).unwrap();
        for state in pre_state {
            match state {
                Db::Integrated(_) => {}
                Db::IntQueue(op) => {
                    let op_hash = DhtOpHashed::with_data(op.clone()).await.into_hash();
                    let val = IntegrationQueueValue {
                        validation_status: ValidationStatus::Valid,
                        op,
                    };
                    workspace
                        .integration_queue
                        .put((TimestampKey::now(), op_hash).try_into().unwrap(), val)
                        .unwrap();
                }
                Db::CasHeader(header, signature) => {
                    let header_hash = HeaderHashed::with_data(header.clone()).await.unwrap();
                    debug!(header_hash = %header_hash.as_hash());
                    let signed_header =
                        SignedHeaderHashed::with_presigned(header_hash, signature.unwrap());
                    workspace.cas.put(signed_header, None).unwrap();
                }
                Db::CasEntry(entry, header, signature) => {
                    let header_hash = HeaderHashed::with_data(header.unwrap().clone())
                        .await
                        .unwrap();
                    let entry_hash = EntryHashed::with_data(entry.clone()).await.unwrap();
                    let signed_header =
                        SignedHeaderHashed::with_presigned(header_hash, signature.unwrap());
                    workspace.cas.put(signed_header, Some(entry_hash)).unwrap();
                }
                Db::MetaHeader(_, _) => {}
                Db::MetaActivity(_) => {}
                Db::MetaUpdate(_, _) => {}
                Db::IntegratedEmpty => {}
                Db::MetaEmpty => {}
                Db::MetaDelete(_, _, _) => {}
                Db::MetaLink(link_add, _) => {
                    workspace.meta.add_link(link_add).await.unwrap();
                }
                Db::MetaLinkEmpty(_) => {}
                Db::IntQueueEmpty => {}
            }
        }
        // Commit workspace
        env_ref
            .with_commit::<WorkspaceError, _, _>(|writer| {
                workspace.flush_to_txn(writer)?;
                Ok(())
            })
            .unwrap();
    }
}

async fn call_workflow<'env>(
    env_ref: &'env EnvironmentReadRef<'env>,
    dbs: &'env impl GetDb,
    env: EnvironmentWrite,
) {
    let reader = env_ref.reader().unwrap();
    let workspace = IntegrateDhtOpsWorkspace::new(&reader, dbs).unwrap();
    let (mut qt, _rx) = TriggerSender::new();
    integrate_dht_ops_workflow(workspace, env.into(), &mut qt)
        .await
        .unwrap();
}

// Need to clear the data from the previous test
fn clear_dbs<'env>(env_ref: &'env EnvironmentWriteRef<'env>, dbs: &'env impl GetDb) {
    let reader = env_ref.reader().unwrap();
    let mut workspace = IntegrateDhtOpsWorkspace::new(&reader, dbs).unwrap();
    env_ref
        .with_commit::<DatabaseError, _, _>(|writer| {
            workspace.integration_queue.clear_all(writer)?;
            workspace.integrated_dht_ops.clear_all(writer)?;
            workspace.cas.clear_all(writer)?;
            workspace.meta.clear_all(writer)?;
            Ok(())
        })
        .unwrap();
}

// TESTS BEGIN HERE
// The following show an op or ops that you want to test
// with a desired pre-state that you want the database in
// and the expected state of the database after the workflow is run

fn store_element(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let entry = match &a.any_header {
        Header::EntryCreate(_) | Header::EntryUpdate(_) => Some(a.original_entry.clone().into()),
        _ => None,
    };
    let op = DhtOp::StoreElement(
        a.signature.clone(),
        a.any_header.clone().into(),
        entry.clone(),
    );
    let pre_state = vec![Db::IntQueue(op.clone())];
    let mut expect = vec![
        Db::Integrated(op.clone()),
        Db::CasHeader(a.any_header.clone().into(), None),
    ];
    if let Some(_) = &entry {
        expect.push(Db::CasEntry(a.original_entry.clone(), None, None));
    }
    (pre_state, expect, "store element")
}

fn store_entry(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::StoreEntry(
        a.signature.clone(),
        a.original_header.clone(),
        a.original_entry.clone().into(),
    );
    let pre_state = vec![Db::IntQueue(op.clone())];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::CasHeader(a.original_header.clone().into(), None),
        Db::CasEntry(a.original_entry.clone(), None, None),
        Db::MetaHeader(a.original_entry.clone(), a.original_header.clone().into()),
    ];
    (pre_state, expect, "store entry")
}

fn register_agent_activity(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterAgentActivity(a.signature.clone(), a.any_header.clone());
    let pre_state = vec![Db::IntQueue(op.clone())];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaActivity(a.any_header.clone()),
    ];
    (pre_state, expect, "register agent activity")
}

fn register_replaced_by_for_header(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterReplacedBy(
        a.signature.clone(),
        a.entry_update_header.clone(),
        Some(a.new_entry.clone().into()),
    );
    let pre_state = vec![Db::IntQueue(op.clone())];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaUpdate(
            a.original_header_hash.clone().into(),
            a.entry_update_header.clone().into(),
        ),
    ];
    (pre_state, expect, "register replaced by for header")
}

fn register_replaced_by_for_entry(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterReplacedBy(
        a.signature.clone(),
        a.entry_update_entry.clone(),
        Some(a.new_entry.clone().into()),
    );
    let pre_state = vec![
        Db::IntQueue(op.clone()),
        Db::CasHeader(a.original_header.clone().into(), Some(a.signature.clone())),
    ];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaUpdate(
            a.original_entry_hash.clone().into(),
            a.entry_update_entry.clone().into(),
        ),
    ];
    (pre_state, expect, "register replaced by for entry")
}

// Register replaced by without store entry
fn register_replaced_by_missing_entry(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterReplacedBy(
        a.signature.clone(),
        a.entry_update_entry.clone(),
        Some(a.new_entry.clone().into()),
    );
    let pre_state = vec![Db::IntQueue(op.clone())];
    let expect = vec![Db::IntegratedEmpty, Db::IntQueue(op.clone()), Db::MetaEmpty];
    (
        pre_state,
        expect,
        "register replaced by for entry missing entry",
    )
}

fn register_deleted_by(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterDeletedEntryHeader(a.signature.clone(), a.entry_delete.clone());
    let pre_state = vec![
        Db::IntQueue(op.clone()),
        Db::CasHeader(a.original_header.clone().into(), Some(a.signature.clone())),
    ];
    let expect = vec![
        Db::IntQueueEmpty,
        Db::Integrated(op.clone()),
        Db::MetaDelete(
            a.original_entry_hash.clone().into(),
            a.original_header_hash.clone().into(),
            a.entry_delete.clone().into(),
        ),
    ];
    (pre_state, expect, "register deleted by")
}

fn register_deleted_by_missing_entry(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterDeletedEntryHeader(a.signature.clone(), a.entry_delete.clone());
    let pre_state = vec![Db::IntQueue(op.clone())];
    let expect = vec![Db::IntegratedEmpty, Db::IntQueue(op.clone()), Db::MetaEmpty];
    (
        pre_state,
        expect,
        "register deleted by for entry missing entry",
    )
}

fn register_deleted_header_by(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterDeletedBy(a.signature.clone(), a.entry_delete.clone());
    let pre_state = vec![
        Db::IntQueue(op.clone()),
        Db::CasHeader(a.original_header.clone().into(), Some(a.signature.clone())),
    ];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaDelete(
            a.original_entry_hash.clone().into(),
            a.original_header_hash.clone().into(),
            a.entry_delete.clone().into(),
        ),
    ];
    (pre_state, expect, "register deleted header by")
}

fn register_add_link(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterAddLink(a.signature.clone(), a.link_add.clone());
    let pre_state = vec![
        Db::IntQueue(op.clone()),
        Db::CasEntry(
            a.original_entry.clone().into(),
            Some(a.original_header.clone().into()),
            Some(a.signature.clone()),
        ),
    ];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaLink(a.link_add.clone(), a.new_entry_hash.clone().into()),
    ];
    (pre_state, expect, "register link add")
}

fn register_remove_link(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterRemoveLink(a.signature.clone(), a.link_remove.clone());
    let pre_state = vec![
        Db::IntQueue(op.clone()),
        Db::CasHeader(a.link_add.clone().into(), Some(a.signature.clone())),
        Db::CasEntry(
            a.original_entry.clone().into(),
            Some(a.original_header.clone().into()),
            Some(a.signature.clone()),
        ),
        Db::MetaLink(a.link_add.clone(), a.new_entry_hash.clone().into()),
    ];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaLinkEmpty(a.link_add.clone()),
    ];
    (pre_state, expect, "register link remove")
}

// The header isn't stored yet
fn register_remove_link_missing_add_header(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterRemoveLink(a.signature.clone(), a.link_remove.clone());
    let pre_state = vec![
        Db::IntQueue(op.clone()),
        Db::CasEntry(
            a.original_entry.clone().into(),
            Some(a.original_header.clone().into()),
            Some(a.signature.clone()),
        ),
    ];
    let expect = vec![Db::IntegratedEmpty, Db::IntQueue(op.clone()), Db::MetaEmpty];
    (
        pre_state,
        expect,
        "register remove link remove missing add header",
    )
}

// Link add is there but metadata is missing
fn register_remove_link_missing_add_metadata(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterRemoveLink(a.signature.clone(), a.link_remove.clone());
    let pre_state = vec![
        Db::IntQueue(op.clone()),
        Db::CasHeader(a.link_add.clone().into(), Some(a.signature.clone())),
        Db::CasEntry(
            a.original_entry.clone().into(),
            Some(a.original_header.clone().into()),
            Some(a.signature.clone()),
        ),
    ];
    let expect = vec![Db::IntegratedEmpty, Db::IntQueue(op.clone()), Db::MetaEmpty];
    (
        pre_state,
        expect,
        "register remove link remove missing add metadata",
    )
}

// Link remove when not an author
fn register_remove_link_missing_base(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterRemoveLink(a.signature.clone(), a.link_remove.clone());
    let pre_state = vec![Db::IntQueue(op.clone())];
    let expect = vec![Db::IntegratedEmpty, Db::IntQueue(op.clone()), Db::MetaEmpty];
    (
        pre_state,
        expect,
        "register remove link remove missing base",
    )
}

// This runs the above tests
#[tokio::test(threaded_scheduler)]
async fn test_ops_state() {
    observability::test_run().ok();
    let env = test_cell_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;

    let tests = [
        store_element,
        store_entry,
        register_agent_activity,
        register_replaced_by_for_header,
        register_replaced_by_for_entry,
        register_replaced_by_missing_entry,
        register_deleted_by,
        register_deleted_by_missing_entry,
        register_deleted_header_by,
        register_add_link,
        register_remove_link,
        register_remove_link_missing_add_header,
        register_remove_link_missing_add_metadata,
        register_remove_link_missing_base,
    ];

    for t in tests.iter() {
        clear_dbs(&env_ref, &dbs);
        let td = TestData::new().await;
        let (pre_state, expect, name) = t(td);
        Db::set(pre_state, &env_ref, &dbs).await;
        call_workflow(&env_ref, &dbs, env.clone()).await;
        Db::check(expect, &env_ref, &dbs, format!("{}: {}", name, here!(""))).await;
    }
}

/// Call the produce dht ops workflow
async fn produce_dht_ops<'env>(
    env_ref: &'env EnvironmentWriteRef<'env>,
    env: EnvironmentWrite,
    dbs: &impl GetDb,
) {
    let (mut qt, _rx) = TriggerSender::new();
    let reader = env_ref.reader().unwrap();
    let workspace = ProduceDhtOpsWorkspace::new(&reader, dbs).unwrap();
    produce_dht_ops_workflow(workspace, env.into(), &mut qt)
        .await
        .unwrap();
}

/// Run genesis on the source chain
async fn genesis<'env>(env_ref: &'env EnvironmentWriteRef<'env>, dbs: &impl GetDb) {
    let reader = env_ref.reader().unwrap();
    let mut workspace = InvokeZomeWorkspace::new(&reader, dbs).unwrap();
    fake_genesis(&mut workspace.source_chain).await.unwrap();
    env_ref
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();
}

async fn commit_entry<'env>(
    pre_state: Vec<Db>,
    env_ref: &'env EnvironmentWriteRef<'env>,
    dbs: &impl GetDb,
    zome_name: ZomeName,
) -> EntryHash {
    let reader = env_ref.reader().unwrap();
    let mut workspace = InvokeZomeWorkspace::new(&reader, dbs).unwrap();

    // Create entry def with the correct zome name
    let entry_def_id = fixt!(EntryDefId);
    let mut entry_def = fixt!(EntryDef);
    entry_def.id = entry_def_id.clone();
    let mut entry_defs_map = BTreeMap::new();
    entry_defs_map.insert(
        ZomeName::from(zome_name.clone()),
        EntryDefs::from(vec![entry_def]),
    );

    // Create a dna file with the correct zome name in the desired position (ZomeId)
    let mut dna_file = DnaFileFixturator::new(Empty).next().unwrap();
    dna_file.dna.zomes.clear();
    dna_file
        .dna
        .zomes
        .push((zome_name.clone().into(), fixt!(Zome)));

    // Create ribosome mock to return fixtures
    // This is a lot faster then compiling a zome
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_dna_file().return_const(dna_file);

    ribosome
        .expect_run_entry_defs()
        .returning(move |_, _| Ok(EntryDefsResult::Defs(entry_defs_map.clone())));

    let mut call_context = CallContextFixturator::new(fixt::Unpredictable)
        .next()
        .unwrap();
    call_context.zome_name = zome_name.clone();

    // Collect the entry from the pre-state to commit
    let entry = pre_state
        .into_iter()
        .filter_map(|state| match state {
            Db::IntQueue(_) => {
                // Will be provided by triggering the produce workflow
                None
            }
            Db::CasEntry(entry, _, _) => Some(entry),
            _ => unreachable!("This test only needs integration queue and an entry in the cas"),
        })
        .next()
        .unwrap();

    let input = CommitEntryInput::new((entry_def_id.clone(), entry.clone()));

    let output = {
        let (_g, raw_workspace) = UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;
        call_context.host_access = host_access.into();
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::commit_entry::commit_entry(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env_ref
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner().try_into().unwrap()
}

async fn get_entry<'env>(
    env_ref: &'env EnvironmentWriteRef<'env>,
    dbs: &impl GetDb,
    entry_hash: EntryHash,
) -> Option<Entry> {
    let reader = env_ref.reader().unwrap();
    let mut workspace = InvokeZomeWorkspace::new(&reader, dbs).unwrap();

    // Create ribosome mock to return fixtures
    // This is a lot faster then compiling a zome
    let ribosome = MockRibosomeT::new();

    let mut call_context = CallContextFixturator::new(fixt::Unpredictable)
        .next()
        .unwrap();

    let input = GetEntryInput::new((entry_hash.clone().into(), GetOptions));

    let output = {
        let (_g, raw_workspace) = UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;
        call_context.host_access = host_access.into();
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::get_entry::get_entry(ribosome.clone(), call_context.clone(), input).unwrap()
    };
    output.into_inner().try_into().unwrap()
}

async fn link_entries<'env>(
    env_ref: &'env EnvironmentWriteRef<'env>,
    dbs: &impl GetDb,
    base_address: EntryHash,
    target_address: EntryHash,
    zome_name: ZomeName,
    link_tag: LinkTag,
) -> HeaderHash {
    let reader = env_ref.reader().unwrap();
    let mut workspace = InvokeZomeWorkspace::new(&reader, dbs).unwrap();

    // Create data for calls
    let mut dna_file = DnaFileFixturator::new(Empty).next().unwrap();
    dna_file.dna.zomes.clear();
    dna_file
        .dna
        .zomes
        .push((zome_name.clone().into(), fixt!(Zome)));

    // Create ribosome mock to return fixtures
    // This is a lot faster then compiling a zome
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_dna_file().return_const(dna_file);

    let mut call_context = CallContextFixturator::new(fixt::Unpredictable)
        .next()
        .unwrap();
    call_context.zome_name = zome_name.clone();

    // Call link_entries
    let input = LinkEntriesInput::new((base_address.into(), target_address.into(), link_tag));

    let output = {
        let (_g, raw_workspace) = UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;
        call_context.host_access = host_access.into();
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        // Call the real link_entries host fn
        host_fn::link_entries::link_entries(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write the changes
    env_ref
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    // Get the LinkAdd HeaderHash back
    unwrap_to!(output.into_inner() => HoloHashCore::HeaderHash)
        .clone()
        .into()
}

async fn get_links<'env>(
    env_ref: &'env EnvironmentWriteRef<'env>,
    dbs: &impl GetDb,
    base_address: EntryHash,
    zome_name: ZomeName,
    link_tag: LinkTag,
) -> Links {
    let reader = env_ref.reader().unwrap();
    let mut workspace = InvokeZomeWorkspace::new(&reader, dbs).unwrap();

    // Create data for calls
    let mut dna_file = DnaFileFixturator::new(Empty).next().unwrap();
    dna_file.dna.zomes.clear();
    dna_file
        .dna
        .zomes
        .push((zome_name.clone().into(), fixt!(Zome)));

    // Create ribosome mock to return fixtures
    // This is a lot faster then compiling a zome
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_dna_file().return_const(dna_file);

    let mut call_context = CallContextFixturator::new(fixt::Unpredictable)
        .next()
        .unwrap();
    call_context.zome_name = zome_name.clone();

    // Call get links
    let input = GetLinksInput::new((base_address.into(), Some(link_tag)));

    let output = {
        let (_g, raw_workspace) = UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;
        call_context.host_access = host_access.into();
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::get_links::get_links(ribosome.clone(), call_context.clone(), input)
            .unwrap()
            .into_inner()
    };

    output
}

// This test is designed to run like the
// register_add_link test except all the
// pre-state is added through real host fn calls
#[tokio::test(threaded_scheduler)]
async fn test_metadata_from_wasm_api() {
    // test workspace boilerplate
    observability::test_run().ok();
    let env = holochain_state::test_utils::test_cell_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;
    clear_dbs(&env_ref, &dbs);

    // Generate fixture data
    let mut td = TestData::with_app_entry_type().await;
    // Only one zome in this test
    td.link_add.zome_id = 0.into();
    let link_tag = td.link_add.tag.clone();
    let target_entry_hash = td.new_entry_hash.clone();
    let zome_name = fixt!(ZomeName);

    // Get db states for an add link op
    let (pre_state, _expect, _) = register_add_link(td);

    // Setup the source chain
    genesis(&env_ref, &dbs).await;

    // Commit the base
    let base_address = commit_entry(pre_state, &env_ref, &dbs, zome_name.clone()).await;

    // Link the base to the target
    let _link_add_address = link_entries(
        &env_ref,
        &dbs,
        base_address.clone(),
        target_entry_hash.clone(),
        zome_name.clone(),
        link_tag.clone(),
    )
    .await;

    // Trigger the produce workflow
    produce_dht_ops(&env_ref, env.clone().into(), &dbs).await;

    // Call integrate
    call_workflow(&env_ref, &dbs, env.clone()).await;

    // Call get links and get back the targets
    let links = get_links(&env_ref, &dbs, base_address, zome_name, link_tag).await;
    let links = links
        .into_inner()
        .into_iter()
        .map(|h| h.target.try_into().unwrap())
        .collect::<Vec<EntryHash>>();

    // Check we only go a single link
    assert_eq!(links.len(), 1);
    // Check we got correct target_entry_hash
    assert_eq!(links[0], target_entry_hash);
    // TODO: create the expect from the result of the commit and link entries
    // Db::check(
    //     expect,
    //     &env_ref,
    //     &dbs,
    //     format!("{}: {}", "metadata from wasm", here!("")),
    // )
    // .await;
}

// This doesn't work without inline integration
#[ignore]
#[tokio::test(threaded_scheduler)]
async fn test_wasm_api_without_integration_links() {
    // test workspace boilerplate
    observability::test_run().ok();
    let env = holochain_state::test_utils::test_cell_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;
    clear_dbs(&env_ref, &dbs);

    // Generate fixture data
    let mut td = TestData::with_app_entry_type().await;
    // Only one zome in this test
    td.link_add.zome_id = 0.into();
    let link_tag = td.link_add.tag.clone();
    let target_entry_hash = td.new_entry_hash.clone();
    let zome_name = fixt!(ZomeName);

    // Get db states for an add link op
    let (pre_state, _expect, _) = register_add_link(td);

    // Setup the source chain
    genesis(&env_ref, &dbs).await;

    // Commit the base
    let base_address = commit_entry(pre_state, &env_ref, &dbs, zome_name.clone()).await;

    // Link the base to the target
    let _link_add_address = link_entries(
        &env_ref,
        &dbs,
        base_address.clone(),
        target_entry_hash.clone(),
        zome_name.clone(),
        link_tag.clone(),
    )
    .await;

    // Call get links and get back the targets
    let links = get_links(&env_ref, &dbs, base_address, zome_name, link_tag).await;
    let links = links
        .into_inner()
        .into_iter()
        .map(|h| h.target.try_into().unwrap())
        .collect::<Vec<EntryHash>>();

    // Check we only go a single link
    assert_eq!(links.len(), 1);
    // Check we got correct target_entry_hash
    assert_eq!(links[0], target_entry_hash);
}

// This doesn't work without inline integration
#[ignore]
#[tokio::test(threaded_scheduler)]
async fn test_wasm_api_without_integration_delete() {
    // test workspace boilerplate
    observability::test_run().ok();
    let env = holochain_state::test_utils::test_cell_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;
    clear_dbs(&env_ref, &dbs);

    // Generate fixture data
    let mut td = TestData::with_app_entry_type().await;
    // Only one zome in this test
    td.link_add.zome_id = 0.into();
    let original_entry = td.original_entry.clone();
    let zome_name = fixt!(ZomeName);

    // Get db states for an add link op
    let (pre_state, _expect, _) = register_add_link(td.clone());

    // Setup the source chain
    genesis(&env_ref, &dbs).await;

    // Commit the base
    let base_address = commit_entry(pre_state.clone(), &env_ref, &dbs, zome_name.clone()).await;

    // Trigger the produce workflow
    produce_dht_ops(&env_ref, env.clone().into(), &dbs).await;

    // Call integrate
    call_workflow(&env_ref, &dbs, env.clone()).await;

    {
        let reader = env_ref.reader().unwrap();
        let mut workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
        let entry_header = workspace
            .meta
            .get_headers(base_address.clone())
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let delete = builder::ElementDelete {
            removes_address: entry_header,
        };
        workspace.source_chain.put(delete, None).await.unwrap();
        env_ref
            .with_commit(|writer| workspace.flush_to_txn(writer))
            .unwrap();
    }
    // Trigger the produce workflow
    produce_dht_ops(&env_ref, env.clone().into(), &dbs).await;

    // Call integrate
    call_workflow(&env_ref, &dbs, env.clone()).await;
    assert_eq!(get_entry(&env_ref, &dbs, base_address.clone()).await, None);
    let base_address = commit_entry(pre_state, &env_ref, &dbs, zome_name.clone()).await;
    assert_eq!(
        get_entry(&env_ref, &dbs, base_address.clone()).await,
        Some(original_entry)
    );
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn test_integrate_single_register_replaced_by_for_header() {
    // For RegisterReplacedBy with intended_for Header
    // metadata has EntryUpdate on HeaderHash but not EntryHash
    todo!()
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn test_integrate_single_register_replaced_by_for_entry() {
    // For RegisterReplacedBy with intended_for Entry
    // metadata has EntryUpdate on EntryHash but not HeaderHash
    todo!()
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn test_integrate_single_register_delete_on_headerd_by() {
    // For RegisterDeletedBy
    // metadata has ElementDelete on HeaderHash
    todo!()
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn test_integrate_single_register_add_link() {
    // For RegisterAddLink
    // metadata has link on EntryHash
    todo!()
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn test_integrate_single_register_remove_link() {
    // For RegisterAddLink
    // metadata has link on EntryHash
    todo!()
}

#[cfg(feature = "slow_tests")]
mod slow_tests {

    use super::*;

    // TODO: Document this test
    // TODO: Use the wasm calls directly instead of setting the databases to
    // a state
    // Integration
    #[tokio::test(threaded_scheduler)]
    async fn commit_entry_add_link() {
        use crate::conductor::{
            api::{
                AdminInterfaceApi, AdminRequest, AdminResponse, AppInterfaceApi, AppRequest,
                RealAdminInterfaceApi, RealAppInterfaceApi,
            },
            ConductorBuilder,
        };
        use crate::core::ribosome::{NamedInvocation, ZomeCallInvocationFixturator};
        use holochain_state::{
            buffer::BufferedStore,
            env::{ReadManager, WriteManager},
            test_utils::{test_conductor_env, test_wasm_env, TestEnvironment},
        };
        use holochain_types::{
            app::{InstallAppDnaPayload, InstallAppPayload},
            fixt::*,
            header::{builder, EntryType},
            observability,
            test_utils::{fake_agent_pubkey_1, fake_dna_zomes, write_fake_dna_file},
            Entry, EntryHashed,
        };
        use holochain_wasm_test_utils::TestWasm;
        use holochain_zome_types::HostInput;
        use matches::assert_matches;
        use uuid::Uuid;

        observability::test_run().ok();
        let test_env = test_conductor_env();
        let _tmpdir = test_env.tmpdir.clone();
        let TestEnvironment {
            env: wasm_env,
            tmpdir: _tmpdir,
        } = test_wasm_env();
        let conductor = ConductorBuilder::new()
            .test(test_env, wasm_env)
            .await
            .unwrap();
        let shutdown = conductor.take_shutdown_handle().await.unwrap();
        let interface = RealAdminInterfaceApi::new(conductor.clone());
        let app_interface = RealAppInterfaceApi::new(conductor.clone());

        // Create dna
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );

        // Install Dna
        let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
        let dna_payload = InstallAppDnaPayload::path_only(fake_dna_path, "".to_string());
        let agent_key = fake_agent_pubkey_1();
        let payload = InstallAppPayload {
            dnas: vec![dna_payload],
            app_id: "test".to_string(),
            agent_key: agent_key.clone(),
        };
        let request = AdminRequest::InstallApp(Box::new(payload));
        let r = interface.handle_admin_request(request).await;
        debug!(?r);
        let installed_app = unwrap_to!(r => AdminResponse::AppInstalled).clone();

        let cell_id = installed_app.cell_data[0].as_id().clone();
        // Activate app
        let request = AdminRequest::ActivateApp {
            app_id: installed_app.app_id,
        };
        let r = interface.handle_admin_request(request).await;
        assert_matches!(r, AdminResponse::AppActivated);

        let mut entry_fixt = SerializedBytesFixturator::new(Predictable).map(|b| Entry::App(b));

        let base_entry = entry_fixt.next().unwrap();
        let base_entry_hash = EntryHashed::with_data(base_entry.clone())
            .await
            .unwrap()
            .into_hash();
        let target_entry = entry_fixt.next().unwrap();
        let target_entry_hash = EntryHashed::with_data(target_entry.clone())
            .await
            .unwrap()
            .into_hash();
        // Put commit entry into source chain
        {
            let cell_env = conductor.get_cell_env(&cell_id).await.unwrap();
            let dbs = cell_env.dbs().await;
            let env_ref = cell_env.guard().await;

            let reader = env_ref.reader().unwrap();
            let mut sc = crate::core::state::source_chain::SourceChain::new(&reader, &dbs).unwrap();

            let header_builder = builder::EntryCreate {
                entry_type: EntryType::App(fixt!(AppEntryType)),
                entry_hash: base_entry_hash.clone(),
            };
            sc.put(header_builder, Some(base_entry.clone()))
                .await
                .unwrap();

            let header_builder = builder::EntryCreate {
                entry_type: EntryType::App(fixt!(AppEntryType)),
                entry_hash: target_entry_hash.clone(),
            };
            sc.put(header_builder, Some(target_entry.clone()))
                .await
                .unwrap();

            let header_builder = builder::LinkAdd {
                base_address: base_entry_hash.clone(),
                target_address: target_entry_hash.clone(),
                zome_id: 0.into(),
                tag: BytesFixturator::new(Unpredictable).next().unwrap().into(),
            };
            sc.put(header_builder, None).await.unwrap();
            env_ref
                .with_commit::<crate::core::state::source_chain::SourceChainError, _, _>(|writer| {
                    sc.flush_to_txn(writer)?;
                    Ok(())
                })
                .unwrap();
        }

        // Call zome to trigger a the produce workflow
        let request = Box::new(
            ZomeCallInvocationFixturator::new(NamedInvocation(
                cell_id.clone(),
                TestWasm::Foo,
                "foo".into(),
                HostInput::new(fixt!(SerializedBytes)),
            ))
            .next()
            .unwrap(),
        );
        let request = AppRequest::ZomeCallInvocation(request);
        let r = app_interface.handle_app_request(request).await;
        debug!(?r);

        tokio::time::delay_for(std::time::Duration::from_secs(4)).await;

        // Check the ops
        {
            let cell_env = conductor.get_cell_env(&cell_id).await.unwrap();
            let dbs = cell_env.dbs().await;
            let env_ref = cell_env.guard().await;

            let reader = env_ref.reader().unwrap();
            let db = dbs.get_db(&*INTEGRATED_DHT_OPS).unwrap();
            let ops_db = IntegratedDhtOpsStore::new(&reader, db).unwrap();
            let ops = ops_db.iter().unwrap().collect::<Vec<_>>().unwrap();
            debug!(?ops);
            assert!(!ops.is_empty());

            let meta = MetadataBuf::primary(&reader, &dbs).unwrap();
            let key = LinkMetaKey::Base(&base_entry_hash);
            let links = meta.get_links(&key).unwrap();
            let link = links[0].clone();
            assert_eq!(link.target, target_entry_hash);

            let (cas, _metadata, cache, metadata_cache) =
                crate::core::state::cascade::test_dbs_and_mocks(&reader, &dbs);
            let cascade =
                crate::core::state::cascade::Cascade::new(&cas, &meta, &cache, &metadata_cache);

            let links = cascade.dht_get_links(&key).await.unwrap();
            let link = links[0].clone();
            assert_eq!(link.target, target_entry_hash);

            let e = cascade.dht_get(&target_entry_hash).await.unwrap().unwrap();
            assert_eq!(e.into_content(), target_entry);

            let e = cascade.dht_get(&base_entry_hash).await.unwrap().unwrap();
            assert_eq!(e.into_content(), base_entry);
        }
        conductor.shutdown().await;
        shutdown.await.unwrap();
    }
}
