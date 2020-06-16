use crate::fixt::{
    AgentValidationPkgFixturator, ChainCloseFixturator, ChainOpenFixturator, DnaFixturator,
    EntryFixturator, EntryHashFixturator, InitZomesCompleteFixturator, LinkAddFixturator,
    LinkRemoveFixturator,
};
use fixt::prelude::*;
use holo_hash::{HeaderHash, HeaderHashFixturator};
use holochain_keystore::Signature;
use holochain_types::{
    composite_hash::EntryHash,
    dht_op::{ops_from_element, DhtOp},
    element::{ChainElement, SignedHeaderHashed},
    fixt::{
        AppEntryTypeFixturator, HeaderBuilderCommonFixturator, SignatureFixturator,
        UpdatesToFixturator,
    },
    header::{
        builder::{self, HeaderBuilder},
        AgentValidationPkg, ChainClose, ChainOpen, Dna, EntryCreate, EntryType, EntryUpdate,
        HeaderBuilderCommon, InitZomesComplete, LinkAdd, LinkRemove, NewEntryHeader, UpdatesTo,
    },
    observability, Entry, Header, HeaderHashed,
};
use holochain_zome_types::entry_def::EntryVisibility;
use pretty_assertions::assert_eq;
use tracing::*;

struct ChainElementTest {
    pub_entry_type: EntryType,
    priv_entry_type: EntryType,
    entry_hash: EntryHash,
    commons: Box<dyn Iterator<Item = HeaderBuilderCommon>>,
    header_hash: HeaderHash,
    sig: Signature,
    entry: Entry,
    updates_to: UpdatesTo,
    link_add: LinkAdd,
    link_remove: LinkRemove,
    dna: Dna,
    chain_close: ChainClose,
    chain_open: ChainOpen,
    agent_validation_pkg: AgentValidationPkg,
    init_zomes_complete: InitZomesComplete,
}

impl ChainElementTest {
    fn new() -> Option<Self> {
        let pub_entry_type = AppEntryTypeFixturator::new(EntryVisibility::Public)
            .map(|a| EntryType::App(a))
            .next()?;
        let priv_entry_type = AppEntryTypeFixturator::new(EntryVisibility::Private)
            .map(|a| EntryType::App(a))
            .next()?;
        let entry_hash = EntryHashFixturator::new(Unpredictable).next()?;
        let commons = HeaderBuilderCommonFixturator::new(Unpredictable);
        let header_hash = HeaderHashFixturator::new(Unpredictable).next()?;
        let sig = SignatureFixturator::new(Unpredictable).next()?;
        let entry = EntryFixturator::new(Unpredictable).next()?;
        let updates_to = UpdatesToFixturator::new(Unpredictable).next()?;
        let link_add = LinkAddFixturator::new(Unpredictable).next()?;
        let link_remove = LinkRemoveFixturator::new(Unpredictable).next()?;
        let dna = fixt!(Dna);
        let chain_open = fixt!(ChainOpen);
        let chain_close = fixt!(ChainClose);
        let agent_validation_pkg = fixt!(AgentValidationPkg);
        let init_zomes_complete = fixt!(InitZomesComplete);
        Some(Self {
            pub_entry_type,
            priv_entry_type,
            entry_hash,
            commons: Box::new(commons),
            header_hash,
            sig,
            entry,
            updates_to,
            link_add,
            link_remove,
            dna,
            chain_close,
            chain_open,
            agent_validation_pkg,
            init_zomes_complete,
        })
    }

    fn entry_create(&mut self, entry_type: EntryType) -> (EntryCreate, ChainElement) {
        let entry_create = builder::EntryCreate {
            entry_type,
            entry_hash: self.entry_hash.clone(),
        }
        .build(self.commons.next().unwrap());
        let element = self.to_element(entry_create.clone().into(), Some(self.entry.clone()));
        (entry_create, element)
    }

    fn entry_update(&mut self, entry_type: EntryType) -> (EntryUpdate, ChainElement) {
        let entry_update = builder::EntryUpdate {
            updates_to: self.updates_to.clone(),
            entry_type,
            entry_hash: self.entry_hash.clone(),
            replaces_address: self.header_hash.clone().into(),
        }
        .build(self.commons.next().unwrap());
        let element = self.to_element(entry_update.clone().into(), Some(self.entry.clone()));
        (entry_update, element)
    }

    fn pub_entry_create(mut self) -> (ChainElement, Vec<DhtOp>) {
        let (entry_create, element) = self.entry_create(self.pub_entry_type.clone());
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

    fn priv_entry_create(mut self) -> (ChainElement, Vec<DhtOp>) {
        let (entry_create, element) = self.entry_create(self.priv_entry_type.clone());
        let header: Header = entry_create.clone().into();

        let ops = vec![
            DhtOp::StoreElement(self.sig.clone(), header.clone(), None),
            DhtOp::RegisterAgentActivity(self.sig.clone(), header.clone()),
        ];
        (element, ops)
    }

    fn priv_entry_update(mut self) -> (ChainElement, Vec<DhtOp>) {
        let (entry_update, element) = self.entry_update(self.priv_entry_type.clone());
        let header: Header = entry_update.clone().into();

        let ops = vec![
            DhtOp::StoreElement(self.sig.clone(), header.clone(), None),
            DhtOp::RegisterAgentActivity(self.sig.clone(), header.clone()),
            DhtOp::RegisterReplacedBy(self.sig.clone(), entry_update, None),
        ];
        (element, ops)
    }

    fn pub_entry_update(mut self) -> (ChainElement, Vec<DhtOp>) {
        let (entry_update, element) = self.entry_update(self.pub_entry_type.clone());
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
        let entry_delete = builder::EntryDelete {
            removes_address: self.header_hash.clone().into(),
        }
        .build(self.commons.next().unwrap());
        let element = self.to_element(entry_delete.clone().into(), None);
        let header: Header = entry_delete.clone().into();

        let ops = vec![
            DhtOp::StoreElement(self.sig.clone(), header.clone(), None),
            DhtOp::RegisterAgentActivity(self.sig.clone(), header.clone()),
            DhtOp::RegisterDeletedBy(self.sig.clone(), entry_delete),
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

// TODO: This should be unit test on [DhtOp] but can't be due to
// the dependencies
#[tokio::test(threaded_scheduler)]
async fn private_entries() {
    let builder = ChainElementTest::new().unwrap();
    let (private_element, expected) = builder.priv_entry_create();
    let result = ops_from_element(&private_element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new().unwrap();
    let (private_element, expected) = builder.priv_entry_update();
    let result = ops_from_element(&private_element).unwrap();
    assert_eq!(result, expected);
}

#[tokio::test(threaded_scheduler)]
async fn public_entries() {
    observability::test_run().ok();
    let builder = ChainElementTest::new().unwrap();
    let (public_element, expected) = builder.pub_entry_create();
    let result = ops_from_element(&public_element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new().unwrap();
    let (public_element, expected) = builder.pub_entry_update();
    let result = ops_from_element(&public_element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new().unwrap();
    let (public_element, expected) = builder.entry_delete();
    let result = ops_from_element(&public_element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new().unwrap();
    let (public_element, expected) = builder.link_add();
    let result = ops_from_element(&public_element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new().unwrap();
    let (public_element, expected) = builder.link_remove();
    let result = ops_from_element(&public_element).unwrap();
    assert_eq!(result, expected);
    let builder = ChainElementTest::new().unwrap();
    let public_elements = builder.others();
    for (public_element, expected) in public_elements {
        debug!(?public_element);
        let result = ops_from_element(&public_element).unwrap();
        assert_eq!(result, expected);
    }
}
