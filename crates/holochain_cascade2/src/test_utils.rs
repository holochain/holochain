use holo_hash::hash_type::AnyDht;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
use holochain_p2p::actor;
use holochain_p2p::HolochainP2pError;
use holochain_state::insert::set_when_integrated;
use holochain_state::insert::update_op_validation_status;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::env::EnvRead;
use holochain_types::env::EnvWrite;
use holochain_types::header::NewEntryHeader;
use holochain_types::metadata::MetadataSet;
use holochain_types::prelude::ValidationPackageResponse;
use holochain_types::timestamp;
use holochain_zome_types::Create;
use holochain_zome_types::DeterministicGetAgentActivityFilter;
use holochain_zome_types::Element;
use holochain_zome_types::Entry;
use holochain_zome_types::Header;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::Link;
use holochain_zome_types::SignedHeaderHashed;
use holochain_zome_types::Update;
use holochain_zome_types::ValidationStatus;
use holochain_zome_types::{fixt::*, DeterministicGetAgentActivityResponse};

use crate::authority;
use crate::authority::WireDhtOp;
use crate::authority::WireLinkKey;
use crate::authority::WireLinkOps;
use crate::authority::WireOps;
use ::fixt::prelude::*;
use holochain_sqlite::db::WriteManager;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::insert::insert_op;

#[derive(Clone)]
pub struct PassThroughNetwork {
    envs: Vec<EnvRead>,
    authority: bool,
}

impl PassThroughNetwork {
    pub fn authority_for_all(envs: Vec<EnvRead>) -> Self {
        Self {
            envs,
            authority: true,
        }
    }

    pub fn authority_for_nothing(envs: Vec<EnvRead>) -> Self {
        Self {
            envs,
            authority: false,
        }
    }
}

#[derive(Clone)]
pub struct MockNetwork(std::sync::Arc<tokio::sync::Mutex<MockHolochainP2pCellT2>>);

impl MockNetwork {
    pub fn new(mock: MockHolochainP2pCellT2) -> Self {
        Self(std::sync::Arc::new(tokio::sync::Mutex::new(mock)))
    }
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait HolochainP2pCellT2 {
    async fn get_validation_package(
        &mut self,
        request_from: AgentPubKey,
        header_hash: HeaderHash,
    ) -> actor::HolochainP2pResult<ValidationPackageResponse>;

    async fn get(
        &mut self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> actor::HolochainP2pResult<Vec<WireOps>>;

    async fn get_meta(
        &mut self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> actor::HolochainP2pResult<Vec<MetadataSet>>;

    async fn get_links(
        &mut self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> actor::HolochainP2pResult<Vec<WireLinkOps>>;

    async fn get_agent_activity(
        &mut self,
        agent: AgentPubKey,
        query: DeterministicGetAgentActivityFilter,
        options: actor::GetActivityOptions,
    ) -> actor::HolochainP2pResult<Vec<DeterministicGetAgentActivityResponse>>;

    async fn authority_for_hash(
        &mut self,
        dht_hash: holo_hash::AnyDhtHash,
    ) -> actor::HolochainP2pResult<bool>;
}

#[async_trait::async_trait]
impl HolochainP2pCellT2 for PassThroughNetwork {
    async fn get_validation_package(
        &mut self,
        _request_from: AgentPubKey,
        _header_hash: HeaderHash,
    ) -> actor::HolochainP2pResult<ValidationPackageResponse> {
        todo!()
    }

    async fn get(
        &mut self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> actor::HolochainP2pResult<Vec<WireOps>> {
        let mut out = Vec::new();
        match *dht_hash.hash_type() {
            AnyDht::Entry => {
                for env in &self.envs {
                    let r = authority::handle_get_entry(
                        env.clone(),
                        dht_hash.clone().into(),
                        (&options).into(),
                    )
                    .map_err(|e| HolochainP2pError::Other(e.into()))?;
                    out.push(WireOps::Entry(r));
                }
            }
            AnyDht::Header => {
                for env in &self.envs {
                    let r = authority::handle_get_element(env.clone(), dht_hash.clone().into())
                        .map_err(|e| HolochainP2pError::Other(e.into()))?;
                    out.push(WireOps::Element(r));
                }
            }
        }
        Ok(out)
    }
    async fn get_meta(
        &mut self,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: actor::GetMetaOptions,
    ) -> actor::HolochainP2pResult<Vec<MetadataSet>> {
        todo!()
    }
    async fn get_links(
        &mut self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> actor::HolochainP2pResult<Vec<WireLinkOps>> {
        let mut out = Vec::new();
        for env in &self.envs {
            let r = authority::handle_get_links(env.clone(), link_key.clone(), (&options).into())
                .map_err(|e| HolochainP2pError::Other(e.into()))?;
            out.push(r);
        }
        Ok(out)
    }
    async fn get_agent_activity(
        &mut self,
        agent: AgentPubKey,
        query: DeterministicGetAgentActivityFilter,
        options: actor::GetActivityOptions,
    ) -> actor::HolochainP2pResult<Vec<DeterministicGetAgentActivityResponse>> {
        let mut out = Vec::new();
        for env in &self.envs {
            let r = authority::handle_get_agent_activity(
                env.clone(),
                agent.clone(),
                query.clone(),
                (&options).into(),
            )
            .map_err(|e| HolochainP2pError::Other(e.into()))?;
            out.push(r);
        }
        Ok(out)
    }

    async fn authority_for_hash(
        &mut self,
        _dht_hash: holo_hash::AnyDhtHash,
    ) -> actor::HolochainP2pResult<bool> {
        Ok(self.authority)
    }
}

#[derive(Debug)]
pub struct EntryTestData {
    pub store_entry_op: DhtOpHashed,
    pub wire_create: WireDhtOp,
    pub create_hash: HeaderHash,
    pub delete_entry_header_op: DhtOpHashed,
    pub wire_delete: WireDhtOp,
    pub delete_hash: HeaderHash,
    pub update_content_op: DhtOpHashed,
    pub wire_update: WireDhtOp,
    pub update_hash: HeaderHash,
    pub hash: EntryHash,
    pub entry: Entry,
    // Links
    pub create_link_op: DhtOpHashed,
    pub delete_link_op: DhtOpHashed,
    pub wire_create_link: WireDhtOp,
    pub wire_delete_link: WireDhtOp,
    pub link_key: WireLinkKey,
    pub link_key_tag: WireLinkKey,
    pub links: Vec<Link>,
}

#[derive(Debug)]
pub struct ElementTestData {
    pub store_element_op: DhtOpHashed,
    pub wire_create: WireDhtOp,
    pub create_hash: HeaderHash,
    pub deleted_by_op: DhtOpHashed,
    pub wire_delete: WireDhtOp,
    pub delete_hash: HeaderHash,
    pub update_element_op: DhtOpHashed,
    pub wire_update: WireDhtOp,
    pub update_hash: HeaderHash,
    pub hash: EntryHash,
    pub entry: Entry,
    pub any_store_element_op: DhtOpHashed,
    pub any_header: WireDhtOp,
    pub any_header_hash: HeaderHash,
    pub any_entry: Option<Entry>,
    pub any_entry_hash: Option<EntryHash>,
    pub any_element: Element,
}

impl EntryTestData {
    pub fn new() -> Self {
        let mut create = fixt!(Create);
        let mut update = fixt!(Update);
        let mut delete = fixt!(Delete);

        let mut create_link = fixt!(CreateLink);
        let mut delete_link = fixt!(DeleteLink);

        let entry = fixt!(AppEntryBytes);
        let entry = Entry::App(entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        let update_entry = fixt!(AppEntryBytes);
        let update_entry = Entry::App(update_entry);
        let update_entry_hash = EntryHash::with_data_sync(&update_entry);

        create.entry_hash = entry_hash.clone();
        update.entry_hash = update_entry_hash.clone();

        let create_header = Header::Create(create.clone());
        let create_hash = HeaderHash::with_data_sync(&create_header);

        delete.deletes_entry_address = entry_hash.clone();
        delete.deletes_address = create_hash.clone();

        update.original_entry_address = entry_hash.clone();
        update.original_header_address = create_hash.clone();

        create_link.base_address = entry_hash.clone();
        delete_link.base_address = entry_hash.clone();
        let create_link_header = Header::CreateLink(create_link.clone());
        let delete_header = Header::Delete(delete.clone());
        let update_header = Header::Update(update.clone());
        let delete_hash = HeaderHash::with_data_sync(&delete_header);
        let update_hash = HeaderHash::with_data_sync(&update_header);

        let create_link_hash = HeaderHash::with_data_sync(&create_link_header);
        delete_link.link_add_address = create_link_hash.clone();
        let delete_link_header = Header::DeleteLink(delete_link.clone());

        let signature = fixt!(Signature);
        let store_entry_op = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryHeader::Create(create.clone()),
            Box::new(entry.clone()),
        ));

        let wire_create = WireDhtOp {
            op_type: store_entry_op.as_content().get_type(),
            header: create_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let signature = fixt!(Signature);
        let delete_entry_header_op = DhtOpHashed::from_content_sync(
            DhtOp::RegisterDeletedEntryHeader(signature.clone(), delete.clone()),
        );

        let wire_delete = WireDhtOp {
            op_type: delete_entry_header_op.as_content().get_type(),
            header: delete_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let signature = fixt!(Signature);
        let update_content_op = DhtOpHashed::from_content_sync(DhtOp::RegisterUpdatedContent(
            signature.clone(),
            update.clone(),
            Some(Box::new(update_entry)),
        ));
        let wire_update = WireDhtOp {
            op_type: update_content_op.as_content().get_type(),
            header: update_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let signature = fixt!(Signature);
        let create_link_op = DhtOpHashed::from_content_sync(DhtOp::RegisterAddLink(
            signature.clone(),
            create_link.clone(),
        ));
        let wire_create_link = WireDhtOp {
            op_type: create_link_op.as_content().get_type(),
            header: create_link_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let signature = fixt!(Signature);
        let delete_link_op = DhtOpHashed::from_content_sync(DhtOp::RegisterRemoveLink(
            signature.clone(),
            delete_link.clone(),
        ));
        let wire_delete_link = WireDhtOp {
            op_type: delete_link_op.as_content().get_type(),
            header: delete_link_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let link_key = WireLinkKey {
            base: create_link.base_address.clone(),
            zome_id: create_link.zome_id,
            tag: None,
        };
        let link_key_tag = WireLinkKey {
            base: create_link.base_address.clone(),
            zome_id: create_link.zome_id,
            tag: Some(create_link.tag.clone()),
        };

        let link = Link {
            target: create_link.target_address.clone(),
            timestamp: create_link.timestamp.clone(),
            tag: create_link.tag.clone(),
            create_link_hash: create_link_hash.clone(),
        };

        Self {
            store_entry_op,
            delete_entry_header_op,
            update_content_op,
            hash: entry_hash,
            entry,
            wire_create,
            wire_delete,
            wire_update,
            create_hash,
            delete_hash,
            update_hash,
            create_link_op,
            delete_link_op,
            wire_create_link,
            wire_delete_link,
            link_key,
            link_key_tag,
            links: vec![link],
        }
    }
}

impl ElementTestData {
    pub fn new() -> Self {
        let mut create = fixt!(Create);
        let mut update = fixt!(Update);
        let mut delete = fixt!(Delete);
        let mut any_header = fixt!(Header);
        let entry = fixt!(AppEntryBytes);
        let entry = Entry::App(entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        let update_entry = fixt!(AppEntryBytes);
        let update_entry = Entry::App(update_entry);
        let update_entry_hash = EntryHash::with_data_sync(&update_entry);

        create.entry_hash = entry_hash.clone();
        update.entry_hash = update_entry_hash.clone();

        let create_header = Header::Create(create.clone());
        let create_hash = HeaderHash::with_data_sync(&create_header);

        delete.deletes_address = create_hash.clone();
        delete.deletes_entry_address = entry_hash.clone();

        update.original_entry_address = entry_hash.clone();
        update.original_header_address = create_hash.clone();

        let delete_header = Header::Delete(delete.clone());
        let update_header = Header::Update(update.clone());
        let delete_hash = HeaderHash::with_data_sync(&delete_header);
        let update_hash = HeaderHash::with_data_sync(&update_header);

        let signature = fixt!(Signature);
        let store_element_op = DhtOpHashed::from_content_sync(DhtOp::StoreElement(
            signature.clone(),
            create_header.clone(),
            Some(Box::new(entry.clone())),
        ));

        let wire_create = WireDhtOp {
            op_type: store_element_op.as_content().get_type(),
            header: create_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let signature = fixt!(Signature);
        let deleted_by_op = DhtOpHashed::from_content_sync(DhtOp::RegisterDeletedBy(
            signature.clone(),
            delete.clone(),
        ));

        let wire_delete = WireDhtOp {
            op_type: deleted_by_op.as_content().get_type(),
            header: delete_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let signature = fixt!(Signature);
        let update_element_op = DhtOpHashed::from_content_sync(DhtOp::RegisterUpdatedElement(
            signature.clone(),
            update.clone(),
            Some(Box::new(update_entry.clone())),
        ));
        let wire_update = WireDhtOp {
            op_type: update_element_op.as_content().get_type(),
            header: update_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let mut any_entry = None;
        let mut any_entry_hash = None;
        if any_header.entry_hash().is_some() {
            match &mut any_header {
                Header::Create(Create { entry_hash: eh, .. })
                | Header::Update(Update { entry_hash: eh, .. }) => {
                    let entry = fixt!(AppEntryBytes);
                    let entry = Entry::App(entry);
                    *eh = EntryHash::with_data_sync(&entry);
                    any_entry_hash = Some(eh.clone());
                    any_entry = Some(Box::new(entry));
                }
                _ => unreachable!(),
            }
        }

        let any_header_hash = HeaderHash::with_data_sync(&any_header);

        let signature = fixt!(Signature);
        let any_store_element_op = DhtOpHashed::from_content_sync(DhtOp::StoreElement(
            signature.clone(),
            any_header.clone(),
            any_entry.clone(),
        ));

        let any_element = Element::new(
            SignedHeaderHashed::with_presigned(
                HeaderHashed::from_content_sync(any_header.clone()),
                signature.clone(),
            ),
            any_entry.clone().map(|i| *i),
        );

        let any_header = WireDhtOp {
            op_type: any_store_element_op.as_content().get_type(),
            header: any_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        Self {
            store_element_op,
            deleted_by_op,
            update_element_op,
            hash: entry_hash,
            entry,
            wire_create,
            wire_delete,
            wire_update,
            create_hash,
            delete_hash,
            update_hash,
            any_store_element_op,
            any_header,
            any_header_hash,
            any_entry: any_entry.map(|e| *e),
            any_entry_hash,
            any_element,
        }
    }
}
pub fn fill_db(env: &EnvWrite, op: DhtOpHashed) {
    env.conn()
        .unwrap()
        .with_commit(|txn| {
            let hash = op.as_hash().clone();
            insert_op(txn, op, false);
            update_op_validation_status(txn, hash.clone(), ValidationStatus::Valid);
            set_when_integrated(txn, hash, timestamp::now());
            DatabaseResult::Ok(())
        })
        .unwrap();
}

pub fn fill_db_rejected(env: &EnvWrite, op: DhtOpHashed) {
    env.conn()
        .unwrap()
        .with_commit(|txn| {
            let hash = op.as_hash().clone();
            insert_op(txn, op, false);
            update_op_validation_status(txn, hash.clone(), ValidationStatus::Rejected);
            set_when_integrated(txn, hash, timestamp::now());
            DatabaseResult::Ok(())
        })
        .unwrap();
}

pub fn fill_db_pending(env: &EnvWrite, op: DhtOpHashed) {
    env.conn()
        .unwrap()
        .with_commit(|txn| {
            let hash = op.as_hash().clone();
            insert_op(txn, op, false);
            update_op_validation_status(txn, hash.clone(), ValidationStatus::Valid);
            DatabaseResult::Ok(())
        })
        .unwrap();
}

pub fn fill_db_as_author(env: &EnvWrite, op: DhtOpHashed) {
    env.conn()
        .unwrap()
        .with_commit(|txn| {
            insert_op(txn, op, true);
            DatabaseResult::Ok(())
        })
        .unwrap();
}

#[async_trait::async_trait]
impl HolochainP2pCellT2 for MockNetwork {
    async fn get_validation_package(
        &mut self,
        request_from: AgentPubKey,
        header_hash: HeaderHash,
    ) -> actor::HolochainP2pResult<ValidationPackageResponse> {
        self.0
            .lock()
            .await
            .get_validation_package(request_from, header_hash)
            .await
    }

    async fn get(
        &mut self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> actor::HolochainP2pResult<Vec<WireOps>> {
        self.0.lock().await.get(dht_hash, options).await
    }

    async fn get_meta(
        &mut self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> actor::HolochainP2pResult<Vec<MetadataSet>> {
        self.0.lock().await.get_meta(dht_hash, options).await
    }

    async fn get_links(
        &mut self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> actor::HolochainP2pResult<Vec<WireLinkOps>> {
        self.0.lock().await.get_links(link_key, options).await
    }

    async fn get_agent_activity(
        &mut self,
        agent: AgentPubKey,
        query: DeterministicGetAgentActivityFilter,
        options: actor::GetActivityOptions,
    ) -> actor::HolochainP2pResult<Vec<DeterministicGetAgentActivityResponse>> {
        self.0
            .lock()
            .await
            .get_agent_activity(agent, query, options)
            .await
    }

    async fn authority_for_hash(
        &mut self,
        dht_hash: holo_hash::AnyDhtHash,
    ) -> actor::HolochainP2pResult<bool> {
        self.0.lock().await.authority_for_hash(dht_hash).await
    }
}

pub fn wire_op_to_shh(op: &WireDhtOp) -> SignedHeaderHashed {
    SignedHeaderHashed::with_presigned(
        HeaderHashed::from_content_sync(op.header.clone()),
        op.signature.clone(),
    )
}
