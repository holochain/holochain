use crate::fixt::AgentValidationPkgFixturator;
use crate::fixt::CloseChainFixturator;
use crate::fixt::CreateFixturator;
use crate::fixt::CreateLinkFixturator;
use crate::fixt::DeleteLinkFixturator;
use crate::fixt::DnaFixturator;
use crate::fixt::EntryFixturator;
use crate::fixt::EntryHashFixturator;
use crate::fixt::EntryTypeFixturator;
use crate::fixt::InitZomesCompleteFixturator;
use crate::fixt::OpenChainFixturator;
use crate::fixt::UpdateFixturator;
use crate::prelude::*;
use ::fixt::prelude::*;
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::*;
use holochain_zome_types::ActionHashed;
use holochain_zome_types::Entry;
use observability;
use tracing::*;

struct ElementTest {
    entry_type: EntryType,
    entry_hash: EntryHash,
    original_entry_hash: EntryHash,
    commons: Box<dyn Iterator<Item = ActionBuilderCommon>>,
    action_hash: ActionHash,
    sig: Signature,
    entry: Entry,
    link_add: CreateLink,
    link_remove: DeleteLink,
    dna: Dna,
    chain_close: CloseChain,
    chain_open: OpenChain,
    agent_validation_pkg: AgentValidationPkg,
    init_zomes_complete: InitZomesComplete,
}

impl ElementTest {
    fn new() -> Self {
        let entry_type = fixt!(EntryType);
        let entry_hash = fixt!(EntryHash);
        let original_entry_hash = fixt!(EntryHash);
        let commons = ActionBuilderCommonFixturator::new(Unpredictable);
        let action_hash = fixt!(ActionHash);
        let sig = fixt!(Signature);
        let entry = fixt!(Entry);
        let link_add = fixt!(CreateLink);
        let link_remove = fixt!(DeleteLink);
        let dna = fixt!(Dna);
        let chain_open = fixt!(OpenChain);
        let chain_close = fixt!(CloseChain);
        let agent_validation_pkg = fixt!(AgentValidationPkg);
        let init_zomes_complete = fixt!(InitZomesComplete);
        Self {
            entry_type,
            entry_hash,
            original_entry_hash,
            commons: Box::new(commons),
            action_hash,
            sig,
            entry,
            link_add,
            link_remove,
            dna,
            chain_close,
            chain_open,
            agent_validation_pkg,
            init_zomes_complete,
        }
    }

    fn create_element(&mut self) -> (Create, Element) {
        let entry_create = builder::Create {
            entry_type: self.entry_type.clone(),
            entry_hash: self.entry_hash.clone(),
        }
        .build(self.commons.next().unwrap());
        let element = self.to_element(entry_create.clone().into(), Some(self.entry.clone()));
        (entry_create, element)
    }

    fn update_element(&mut self) -> (Update, Element) {
        let entry_update = builder::Update {
            original_entry_address: self.original_entry_hash.clone(),
            entry_type: self.entry_type.clone(),
            entry_hash: self.entry_hash.clone(),
            original_action_address: self.action_hash.clone().into(),
        }
        .build(self.commons.next().unwrap());
        let element = self.to_element(entry_update.clone().into(), Some(self.entry.clone()));
        (entry_update, element)
    }

    fn entry_create(&mut self) -> (Element, Vec<DhtOp>) {
        let (entry_create, element) = self.create_element();
        let action: Action = entry_create.clone().into();

        let ops = vec![
            DhtOp::StoreElement(
                self.sig.clone(),
                action.clone(),
                Some(self.entry.clone().into()),
            ),
            DhtOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            DhtOp::StoreEntry(
                self.sig.clone(),
                NewEntryAction::Create(entry_create),
                self.entry.clone().into(),
            ),
        ];
        (element, ops)
    }

    fn entry_update(&mut self) -> (Element, Vec<DhtOp>) {
        let (entry_update, element) = self.update_element();
        let action: Action = entry_update.clone().into();

        let ops = vec![
            DhtOp::StoreElement(
                self.sig.clone(),
                action.clone(),
                Some(self.entry.clone().into()),
            ),
            DhtOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            DhtOp::StoreEntry(
                self.sig.clone(),
                NewEntryAction::Update(entry_update.clone()),
                self.entry.clone().into(),
            ),
            DhtOp::RegisterUpdatedContent(
                self.sig.clone(),
                entry_update.clone(),
                Some(self.entry.clone().into()),
            ),
            DhtOp::RegisterUpdatedElement(
                self.sig.clone(),
                entry_update,
                Some(self.entry.clone().into()),
            ),
        ];
        (element, ops)
    }

    fn entry_delete(&mut self) -> (Element, Vec<DhtOp>) {
        let entry_delete = builder::Delete {
            deletes_address: self.action_hash.clone(),
            deletes_entry_address: self.entry_hash.clone(),
        }
        .build(self.commons.next().unwrap());
        let element = self.to_element(entry_delete.clone().into(), None);
        let action: Action = entry_delete.clone().into();

        let ops = vec![
            DhtOp::StoreElement(self.sig.clone(), action.clone(), None),
            DhtOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            DhtOp::RegisterDeletedBy(self.sig.clone(), entry_delete.clone()),
            DhtOp::RegisterDeletedEntryAction(self.sig.clone(), entry_delete),
        ];
        (element, ops)
    }

    fn link_add(&mut self) -> (Element, Vec<DhtOp>) {
        let element = self.to_element(self.link_add.clone().into(), None);
        let action: Action = self.link_add.clone().into();

        let ops = vec![
            DhtOp::StoreElement(self.sig.clone(), action.clone(), None),
            DhtOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            DhtOp::RegisterAddLink(self.sig.clone(), self.link_add.clone()),
        ];
        (element, ops)
    }

    fn link_remove(&mut self) -> (Element, Vec<DhtOp>) {
        let element = self.to_element(self.link_remove.clone().into(), None);
        let action: Action = self.link_remove.clone().into();

        let ops = vec![
            DhtOp::StoreElement(self.sig.clone(), action.clone(), None),
            DhtOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            DhtOp::RegisterRemoveLink(self.sig.clone(), self.link_remove.clone()),
        ];
        (element, ops)
    }

    fn others(&self) -> Vec<(Element, Vec<DhtOp>)> {
        let mut elements = Vec::new();
        elements.push(self.to_element(self.dna.clone().into(), None));
        elements.push(self.to_element(self.chain_open.clone().into(), None));
        elements.push(self.to_element(self.chain_close.clone().into(), None));
        elements.push(self.to_element(self.agent_validation_pkg.clone().into(), None));
        elements.push(self.to_element(self.init_zomes_complete.clone().into(), None));
        let mut chain_elements = Vec::new();
        for element in elements {
            let action: Action = element.action().clone();

            let ops = vec![
                DhtOp::StoreElement(self.sig.clone(), action.clone(), None),
                DhtOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            ];
            chain_elements.push((element, ops));
        }
        chain_elements
    }

    fn to_element(&self, action: Action, entry: Option<Entry>) -> Element {
        let h = ActionHashed::from_content_sync(action.clone());
        let h = SignedActionHashed::with_presigned(h, self.sig.clone());
        Element::new(h, entry.clone())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_all_ops() {
    observability::test_run().ok();
    let mut builder = ElementTest::new();
    let (element, expected) = builder.entry_create();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let (element, expected) = builder.entry_update();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let (element, expected) = builder.entry_delete();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let (element, expected) = builder.link_add();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let (element, expected) = builder.link_remove();
    let result = produce_ops_from_element(&element).unwrap();
    assert_eq!(result, expected);
    let elements = builder.others();
    for (element, expected) in elements {
        debug!(?element);
        let result = produce_ops_from_element(&element).unwrap();
        assert_eq!(result, expected);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dht_basis() {
    // Create a action that points to an entry
    let original_action = fixt!(Create);
    let expected_entry_hash: AnyDhtHash = original_action.entry_hash.clone().into();

    let original_action_hash =
        ActionHashed::from_content_sync(Action::Create(original_action.clone()));
    let original_action_hash = original_action_hash.into_inner().1;

    // Create the update action with the same hash
    let update_new_entry = fixt!(Entry);
    let mut entry_update = fixt!(Update, update_new_entry.clone());
    entry_update.original_entry_address = original_action.entry_hash.clone();
    entry_update.original_action_address = original_action_hash;

    // Create the op
    let op = DhtOp::RegisterUpdatedContent(
        fixt!(Signature),
        entry_update,
        Some(update_new_entry.into()),
    );

    // Get the basis
    let result = op.dht_basis();

    // Check the hash matches
    assert_eq!(expected_entry_hash, result);
}

fn all_elements() -> Vec<Element> {
    let mut out = Vec::with_capacity(5);
    let mut builder = ElementTest::new();
    let (element, _) = builder.entry_create();
    out.push(element);
    let (element, _) = builder.entry_update();
    out.push(element);
    let (element, _) = builder.entry_delete();
    out.push(element);
    let (element, _) = builder.link_add();
    out.push(element);
    let (element, _) = builder.link_remove();
    out.push(element);
    out
}

#[test]
fn get_type_op() {
    let check_all_ops = |element| {
        let ops = produce_ops_from_element(&element).unwrap();
        let check_type = |op: DhtOp| {
            let op_type = op.get_type();
            assert_eq!(op.to_light().get_type(), op_type);
            match op {
                DhtOp::StoreElement(_, _, _) => assert_eq!(op_type, DhtOpType::StoreElement),
                DhtOp::StoreEntry(_, _, _) => assert_eq!(op_type, DhtOpType::StoreEntry),
                DhtOp::RegisterAgentActivity(_, _) => {
                    assert_eq!(op_type, DhtOpType::RegisterAgentActivity)
                }
                DhtOp::RegisterUpdatedContent(_, _, _) => {
                    assert_eq!(op_type, DhtOpType::RegisterUpdatedContent)
                }
                DhtOp::RegisterUpdatedElement(_, _, _) => {
                    assert_eq!(op_type, DhtOpType::RegisterUpdatedElement)
                }
                DhtOp::RegisterDeletedBy(_, _) => assert_eq!(op_type, DhtOpType::RegisterDeletedBy),
                DhtOp::RegisterDeletedEntryAction(_, _) => {
                    assert_eq!(op_type, DhtOpType::RegisterDeletedEntryAction)
                }
                DhtOp::RegisterAddLink(_, _) => assert_eq!(op_type, DhtOpType::RegisterAddLink),
                DhtOp::RegisterRemoveLink(_, _) => {
                    assert_eq!(op_type, DhtOpType::RegisterRemoveLink)
                }
            }
        };
        for op in ops {
            check_type(op);
        }
    };

    for element in all_elements() {
        check_all_ops(element);
    }
}

#[test]
fn from_type_op() {
    let check_all_ops = |element| {
        let ops = produce_ops_from_element(&element).unwrap();
        let check_identity = |op: DhtOp, action, entry| {
            assert_eq!(DhtOp::from_type(op.get_type(), action, entry).unwrap(), op)
        };
        for op in ops {
            check_identity(
                op,
                SignedAction::from(element.signed_action().clone()),
                element.entry().clone().into_option(),
            );
        }
    };

    for element in all_elements() {
        check_all_ops(element);
    }
}

#[test]
fn from_type_op_light() {
    let check_all_ops = |element| {
        let ops = produce_op_lights_from_elements(vec![&element]).unwrap();
        let check_identity = |light: DhtOpLight, action| {
            let action_hash = ActionHash::with_data_sync(action);
            assert_eq!(
                DhtOpLight::from_type(light.get_type(), action_hash, action).unwrap(),
                light
            )
        };
        for op in ops {
            check_identity(op, element.action());
        }
    };
    for element in all_elements() {
        check_all_ops(element);
    }
}

#[test]
fn test_all_ops_basis() {
    let check_all_ops = |element| {
        let ops = produce_ops_from_element(&element).unwrap();
        let check_basis = |op: DhtOp| match (op.get_type(), op.dht_basis()) {
            (DhtOpType::StoreElement, basis) => {
                assert_eq!(basis, AnyDhtHash::from(element.action_address().clone()))
            }
            (DhtOpType::StoreEntry, basis) => {
                assert_eq!(
                    basis,
                    AnyDhtHash::from(element.action().entry_hash().unwrap().clone())
                )
            }
            (DhtOpType::RegisterAgentActivity, basis) => {
                assert_eq!(basis, AnyDhtHash::from(element.action().author().clone()))
            }
            (DhtOpType::RegisterUpdatedContent, basis) => {
                assert_eq!(
                    basis,
                    AnyDhtHash::from(
                        Update::try_from(element.action().clone())
                            .unwrap()
                            .original_entry_address
                            .clone()
                    )
                )
            }
            (DhtOpType::RegisterUpdatedElement, basis) => {
                assert_eq!(
                    basis,
                    AnyDhtHash::from(
                        Update::try_from(element.action().clone())
                            .unwrap()
                            .original_action_address
                            .clone()
                    )
                )
            }
            (DhtOpType::RegisterDeletedBy, basis) => {
                assert_eq!(
                    basis,
                    AnyDhtHash::from(
                        Delete::try_from(element.action().clone())
                            .unwrap()
                            .deletes_address
                            .clone()
                    )
                )
            }
            (DhtOpType::RegisterDeletedEntryAction, basis) => {
                assert_eq!(
                    basis,
                    AnyDhtHash::from(
                        Delete::try_from(element.action().clone())
                            .unwrap()
                            .deletes_entry_address
                            .clone()
                    )
                )
            }
            (DhtOpType::RegisterAddLink, basis) => {
                assert_eq!(
                    basis,
                    AnyDhtHash::from(
                        CreateLink::try_from(element.action().clone())
                            .unwrap()
                            .base_address
                            .clone()
                    )
                )
            }
            (DhtOpType::RegisterRemoveLink, basis) => {
                assert_eq!(
                    basis,
                    AnyDhtHash::from(
                        DeleteLink::try_from(element.action().clone())
                            .unwrap()
                            .base_address
                            .clone()
                    )
                )
            }
        };
        for op in ops {
            assert_eq!(*op.to_light().dht_basis(), op.dht_basis());
            check_basis(op);
        }
    };
    for element in all_elements() {
        check_all_ops(element);
    }
}
