use super::dht_basis;
use crate::{
    core::state::chain_cas::ChainCasBuf,
    fixt::{
        AgentValidationPkgFixturator, ChainCloseFixturator, ChainOpenFixturator, DnaFixturator,
        EntryCreateFixturator, EntryFixturator, EntryHashFixturator, EntryTypeFixturator,
        EntryUpdateFixturator, InitZomesCompleteFixturator, LinkAddFixturator,
        LinkRemoveFixturator,
    },
};
use ::fixt::prelude::*;
use holo_hash::{fixt::HeaderHashFixturator, *};
use holochain_keystore::Signature;
use holochain_state::{env::ReadManager, test_utils::test_cell_env};
use holochain_types::{
    dht_op::{produce_ops_from_element, DhtOp},
    element::{ChainElement, SignedHeaderHashed},
    fixt::{HeaderBuilderCommonFixturator, IntendedForFixturator, SignatureFixturator},
    header::NewEntryHeader,
    observability, Entry, EntryHashed, HeaderHashed,
};
use holochain_zome_types::header::{
    builder::{self, HeaderBuilder},
    AgentValidationPkg, ChainClose, ChainOpen, Dna, EntryCreate, EntryType, EntryUpdate, Header,
    HeaderBuilderCommon, InitZomesComplete, IntendedFor, LinkAdd, LinkRemove,
};
use pretty_assertions::assert_eq;
use tracing::*;

struct ChainElementTest {
    entry_type: EntryType,
    entry_hash: EntryHash,
    commons: Box<dyn Iterator<Item = HeaderBuilderCommon>>,
    header_hash: HeaderHash,
    sig: Signature,
    entry: Entry,
    intended_for: IntendedFor,
    link_add: LinkAdd,
    link_remove: LinkRemove,
    dna: Dna,
    chain_close: ChainClose,
    chain_open: ChainOpen,
    agent_validation_pkg: AgentValidationPkg,
    init_zomes_complete: InitZomesComplete,
}

impl ChainElementTest {
    fn new() -> Self {
        let entry_type = fixt!(EntryType);
        let entry_hash = fixt!(EntryHash);
        let commons = HeaderBuilderCommonFixturator::new(Unpredictable);
        let header_hash = fixt!(HeaderHash);
        let sig = fixt!(Signature);
        let entry = fixt!(Entry);
        let intended_for = fixt!(IntendedFor);
        let link_add = fixt!(LinkAdd);
        let link_remove = fixt!(LinkRemove);
        let dna = fixt!(Dna);
        let chain_open = fixt!(ChainOpen);
        let chain_close = fixt!(ChainClose);
        let agent_validation_pkg = fixt!(AgentValidationPkg);
        let init_zomes_complete = fixt!(InitZomesComplete);
        Self {
            entry_type,
            entry_hash,
            commons: Box::new(commons),
            header_hash,
            sig,
            entry,
            intended_for,
            link_add,
            link_remove,
            dna,
            chain_close,
            chain_open,
            agent_validation_pkg,
            init_zomes_complete,
        }
    }

    fn create_element(&mut self) -> (EntryCreate, ChainElement) {
        let entry_create = builder::EntryCreate {
            entry_type: self.entry_type.clone(),
            entry_hash: self.entry_hash.clone(),
        }
        .build(self.commons.next().unwrap());
        let element = self.to_element(entry_create.clone().into(), Some(self.entry.clone()));
        (entry_create, element)
    }

    fn update_element(&mut self) -> (EntryUpdate, ChainElement) {
        let entry_update = builder::EntryUpdate {
            intended_for: self.intended_for.clone(),
            entry_type: self.entry_type.clone(),
            entry_hash: self.entry_hash.clone(),
            replaces_address: self.header_hash.clone().into(),
        }
        .build(self.commons.next().unwrap());
        let element = self.to_element(entry_update.clone().into(), Some(self.entry.clone()));
        (entry_update, element)
    }

    fn entry_create(mut self) -> (ChainElement, Vec<DhtOp>) {
        let (entry_create, element) = self.create_element();
        let header: Header = entry_create.clone().into();

        let ops = vec![
            DhtOp::StoreElement(
                self.sig.clone(),
                header.clone(),
                Some(self.entry.clone().into()),
            ),
            DhtOp::RegisterAgentActivity(self.sig.clone(), header.clone()),
            DhtOp::StoreEntry(
                self.sig.clone(),
                NewEntryHeader::Create(entry_create),
                self.entry.clone().into(),
            ),
        ];
        (element, ops)
    }

    fn entry_update(mut self) -> (ChainElement, Vec<DhtOp>) {
        let (entry_update, element) = self.update_element();
        let header: Header = entry_update.clone().into();

        let ops = vec![
            DhtOp::StoreElement(
                self.sig.clone(),
                header.clone(),
                Some(self.entry.clone().into()),
            ),
            DhtOp::RegisterAgentActivity(self.sig.clone(), header.clone()),
            DhtOp::StoreEntry(
                self.sig.clone(),
                NewEntryHeader::Update(entry_update.clone()),
                self.entry.clone().into(),
            ),
            DhtOp::RegisterReplacedBy(
                self.sig.clone(),
                entry_update,
                Some(self.entry.clone().into()),
            ),
        ];
        (element, ops)
    }

    fn entry_delete(mut self) -> (ChainElement, Vec<DhtOp>) {
        let entry_delete = builder::ElementDelete {
            removes_address: self.header_hash.clone().into(),
        }
        .build(self.commons.next().unwrap());
        let element = self.to_element(entry_delete.clone().into(), None);
        let header: Header = entry_delete.clone().into();

        let ops = vec![
            DhtOp::StoreElement(self.sig.clone(), header.clone(), None),
            DhtOp::RegisterAgentActivity(self.sig.clone(), header.clone()),
            DhtOp::RegisterDeletedBy(self.sig.clone(), entry_delete.clone()),
            DhtOp::RegisterDeletedEntryHeader(self.sig, entry_delete),
        ];
        (element, ops)
    }

    fn link_add(mut self) -> (ChainElement, Vec<DhtOp>) {
        let element = self.to_element(self.link_add.clone().into(), None);
        let header: Header = self.link_add.clone().into();

        let ops = vec![
            DhtOp::StoreElement(self.sig.clone(), header.clone(), None),
            DhtOp::RegisterAgentActivity(self.sig.clone(), header.clone()),
            DhtOp::RegisterAddLink(self.sig.clone(), self.link_add.clone()),
        ];
        (element, ops)
    }

    fn link_remove(mut self) -> (ChainElement, Vec<DhtOp>) {
        let element = self.to_element(self.link_remove.clone().into(), None);
        let header: Header = self.link_remove.clone().into();

        let ops = vec![
            DhtOp::StoreElement(self.sig.clone(), header.clone(), None),
            DhtOp::RegisterAgentActivity(self.sig.clone(), header.clone()),
            DhtOp::RegisterRemoveLink(self.sig.clone(), self.link_remove.clone()),
        ];
        (element, ops)
    }

    fn others(mut self) -> Vec<(ChainElement, Vec<DhtOp>)> {
        let mut elements = Vec::new();
        elements.push(self.to_element(self.dna.clone().into(), None));
        elements.push(self.to_element(self.chain_open.clone().into(), None));
        elements.push(self.to_element(self.chain_close.clone().into(), None));
        elements.push(self.to_element(self.agent_validation_pkg.clone().into(), None));
        elements.push(self.to_element(self.init_zomes_complete.clone().into(), None));
        let mut chain_elements = Vec::new();
        for element in elements {
            let header: Header = element.header().clone();

            let ops = vec![
                DhtOp::StoreElement(self.sig.clone(), header.clone(), None),
                DhtOp::RegisterAgentActivity(self.sig.clone(), header.clone()),
            ];
            chain_elements.push((element, ops));
        }
        chain_elements
    }

    fn to_element(&mut self, header: Header, entry: Option<Entry>) -> ChainElement {
        let h = HeaderHashed::with_pre_hashed(header.clone(), self.header_hash.clone());
        let h = SignedHeaderHashed::with_presigned(h, self.sig.clone());
        ChainElement::new(h, entry.clone())
    }
}

#[tokio::test(threaded_scheduler)]
async fn test_all_ops() {
    observability::test_run().ok();
    let builder = ChainElementTest::new();
    let (element, expected) = builder.entry_create();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new();
    let (element, expected) = builder.entry_update();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new();
    let (element, expected) = builder.entry_delete();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new();
    let (element, expected) = builder.link_add();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new();
    let (element, expected) = builder.link_remove();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new();
    let elements = builder.others();
    for (element, expected) in elements {
        debug!(?element);
        let result = produce_ops_from_element(&element).unwrap();
        assert_eq!(result, expected);
    }
}

#[tokio::test(threaded_scheduler)]
async fn test_dht_basis() {
    let env = test_cell_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;

    {
        // Create a header that points to an entry
        let new_entry = fixt!(Entry);
        let original_header = fixt!(EntryCreate);
        let expected_entry_hash: AnyDhtHash = original_header.entry_hash.clone().into();

        let original_header_hash = HeaderHashed::with_data(Header::EntryCreate(original_header))
            .await
            .unwrap();
        let signed_header =
            SignedHeaderHashed::with_presigned(original_header_hash.clone(), fixt!(Signature));
        let original_header_hash = original_header_hash.into_inner().1;

        let entry_hashed = EntryHashed::with_pre_hashed(new_entry.clone(), fixt!(EntryHash));

        // Setup a cascade
        let reader = env_ref.reader().unwrap();
        let mut cas = ChainCasBuf::primary(&reader, &dbs, true).unwrap();

        // Put the header into the db
        cas.put(signed_header, Some(entry_hashed)).unwrap();

        // Create the update header with the same hash
        let mut entry_update = fixt!(EntryUpdate);
        entry_update.intended_for = IntendedFor::Entry;
        entry_update.replaces_address = original_header_hash;

        // Create the op
        let op = DhtOp::RegisterReplacedBy(fixt!(Signature), entry_update, Some(new_entry.into()));

        // Get the basis
        let result = dht_basis(&op, &cas).await.unwrap();

        // Check the hash matches
        assert_eq!(expected_entry_hash, result);
    }
}
