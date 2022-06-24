#![allow(missing_docs)]

use crate::conductor::api::CellConductorApi;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::api::CellConductorReadHandle;
use crate::conductor::interface::SignalBroadcaster;
use crate::conductor::ConductorHandle;
use crate::core::ribosome::host_fn;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallHostAccess;
use crate::core::ribosome::ZomeCallInvocation;
use hdk::prelude::*;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::GetLinksOptions;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_p2p::HolochainP2pDna;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::prelude::*;
use holochain_zome_types::AgentActivity;
use std::sync::Arc;
use unwrap_to::unwrap_to;

use super::CreateInputExt;

// Commit entry types //
// Useful for when you want to commit something
// that will match entry defs
pub const POST_ID: &str = "post";
pub const POST_INDEX: LocalZomeTypeId = LocalZomeTypeId(0);
pub const MSG_ID: &str = "msg";
pub const MSG_INDEX: LocalZomeTypeId = LocalZomeTypeId(1);
pub const VALID_ID: &str = "always_validates";
pub const INVALID_ID: &str = "never_validates";

#[derive(
    Default, Debug, PartialEq, Clone, SerializedBytes, serde::Serialize, serde::Deserialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Post(pub String);
#[derive(
    Default, Debug, PartialEq, Clone, SerializedBytes, serde::Serialize, serde::Deserialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Msg(pub String);

/// A CellId plus ZomeName: the full "path" to a zome in the conductor
#[derive(Clone, Debug, derive_more::From, derive_more::Into)]
pub struct ZomePath(CellId, ZomeName);

impl ZomePath {
    pub fn cell_id(&self) -> &CellId {
        &self.0
    }

    pub fn zome_name(&self) -> &ZomeName {
        &self.1
    }
}

/// Type from the validate wasm
// TODO: Maybe we can dry this up by putting the wasm types
// somewhere outside the wasm?
#[derive(Deserialize, Serialize, SerializedBytes, Debug, Clone)]
pub enum ThisWasmEntry {
    AlwaysValidates,
    NeverValidates,
}

#[derive(Deserialize, Serialize, SerializedBytes, Debug, Clone)]
pub enum MaybeLinkable {
    AlwaysLinkable,
    NeverLinkable,
}

/// A freely callable version of the host fn api, so that host functions
/// can be called from Rust instead of Wasm
#[derive(Clone)]
pub struct HostFnCaller {
    pub authored_db: DbWrite<DbKindAuthored>,
    pub dht_db: DbWrite<DbKindDht>,
    pub dht_db_cache: DhtDbQueryCache,
    pub cache: DbWrite<DbKindCache>,
    pub ribosome: RealRibosome,
    pub zome_path: ZomePath,
    pub network: HolochainP2pDna,
    pub keystore: MetaLairClient,
    pub signal_tx: SignalBroadcaster,
    pub call_zome_handle: CellConductorReadHandle,
}

impl HostFnCaller {
    /// Create HostFnCaller for the first zome.
    // #[deprecated = "use create_for_zome"]
    pub async fn create(
        cell_id: &CellId,
        handle: &ConductorHandle,
        dna_file: &DnaFile,
    ) -> HostFnCaller {
        Self::create_for_zome(cell_id, handle, dna_file, 0).await
    }

    /// Create HostFnCaller for a specific zome if there are multiple.
    pub async fn create_for_zome(
        cell_id: &CellId,
        handle: &ConductorHandle,
        dna_file: &DnaFile,
        zome_index: usize,
    ) -> HostFnCaller {
        let authored_db = handle.get_authored_db(cell_id.dna_hash()).unwrap();
        let dht_db = handle.get_dht_db(cell_id.dna_hash()).unwrap();
        let dht_db_cache = handle.get_dht_db_cache(cell_id.dna_hash()).unwrap();
        let cache = handle.get_cache_db(cell_id).unwrap();
        let keystore = handle.keystore().clone();
        let network = handle.holochain_p2p().to_dna(cell_id.dna_hash().clone());

        let zome_path = (
            cell_id.clone(),
            dna_file
                .dna()
                .integrity_zomes
                .get(zome_index)
                .unwrap()
                .0
                .clone(),
        )
            .into();
        let ribosome = handle.get_ribosome(dna_file.dna_hash()).unwrap();
        let signal_tx = handle.signal_broadcaster().await;
        let call_zome_handle =
            CellConductorApi::new(handle.clone(), cell_id.clone()).into_call_zome_handle();
        HostFnCaller {
            authored_db,
            dht_db,
            dht_db_cache,
            cache,
            ribosome,
            zome_path,
            network,
            keystore,
            signal_tx,
            call_zome_handle,
        }
    }

    pub fn authored_db(&self) -> DbWrite<DbKindAuthored> {
        self.authored_db.clone()
    }

    pub fn dht_db(&self) -> DbWrite<DbKindDht> {
        self.dht_db.clone()
    }

    pub async fn unpack(&self) -> (Arc<RealRibosome>, Arc<CallContext>, HostFnWorkspace) {
        let HostFnCaller {
            authored_db,
            dht_db,
            cache,
            network,
            keystore,
            ribosome,
            signal_tx,
            zome_path,
            call_zome_handle,
            dht_db_cache,
        } = self.clone();

        let (cell_id, zome_name) = zome_path.into();

        let workspace_lock = HostFnWorkspace::new(
            authored_db,
            dht_db,
            dht_db_cache,
            cache,
            keystore.clone(),
            Some(cell_id.agent_pubkey().clone()),
            Arc::new(ribosome.dna_def().as_content().clone()),
        )
        .await
        .unwrap();
        let host_access = ZomeCallHostAccess::new(
            workspace_lock.clone(),
            keystore,
            network,
            signal_tx,
            call_zome_handle,
        );
        let ribosome = Arc::new(ribosome);
        let zome = ribosome.dna_def().get_zome(&zome_name).unwrap();
        let call_context = Arc::new(CallContext::new(
            zome,
            FunctionName::new("not_sure_what_should_be_here"),
            host_access.into(),
            // Auth as the author.
            InvocationAuth::Cap(cell_id.agent_pubkey().clone(), None),
        ));
        (ribosome, call_context, workspace_lock)
    }
}

impl HostFnCaller {
    pub fn get_entry_type(
        &self,
        zome: impl Into<ZomeName>,
        local_index: impl Into<LocalZomeTypeId>,
    ) -> EntryDefIndex {
        let zome_dependencies = self.ribosome.get_zome_dependencies(&zome.into()).unwrap();
        let zome_types = self
            .ribosome
            .zome_types()
            .re_scope(zome_dependencies)
            .unwrap();
        zome_types
            .entries
            .to_global_scope(local_index)
            .unwrap()
            .into()
    }
    pub fn get_link_type(
        &self,
        zome: impl Into<ZomeName>,
        local_index: impl Into<LocalZomeTypeId>,
    ) -> LinkType {
        let zome_dependencies = self.ribosome.get_zome_dependencies(&zome.into()).unwrap();
        let zome_types = self
            .ribosome
            .zome_types()
            .re_scope(zome_dependencies)
            .unwrap();
        zome_types
            .links
            .to_global_scope(local_index)
            .unwrap()
            .into()
    }
    pub async fn commit_entry<E: Into<EntryDefIndex>>(
        &self,
        entry: Entry,
        entry_def_id: E,
        visibility: EntryVisibility,
    ) -> ActionHash {
        let (ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = CreateInput::app_entry(
            entry_def_id.into(),
            visibility,
            entry,
            ChainTopOrdering::default(),
        );
        let output = host_fn::create::create(ribosome, call_context, input).unwrap();

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn delete_entry<'env>(&self, input: DeleteInput) -> ActionHash {
        let (ribosome, call_context, workspace_lock) = self.unpack().await;
        let output = {
            let r = host_fn::delete::delete(ribosome, call_context, input);
            let r = r.map_err(|e| {
                debug!(%e);
                e
            });
            r.unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn update_entry(
        &self,
        entry: Entry,
        original_action_address: ActionHash,
    ) -> ActionHash {
        let (ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = UpdateInput {
            original_action_address,
            entry,
            chain_top_ordering: Default::default(),
        };

        let output = { host_fn::update::update(ribosome, call_context, input).unwrap() };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn get(&self, entry_hash: AnyDhtHash, options: GetOptions) -> Vec<Option<Record>> {
        let (ribosome, call_context, _) = self.unpack().await;
        let input = GetInput::new(entry_hash, options);
        host_fn::get::get(ribosome, call_context, vec![input]).unwrap()
    }

    pub async fn get_details<'env>(
        &self,
        entry_hash: AnyDhtHash,
        options: GetOptions,
    ) -> Vec<Option<Details>> {
        let (ribosome, call_context, _) = self.unpack().await;
        let input = GetInput::new(entry_hash, options);
        host_fn::get_details::get_details(ribosome, call_context, vec![input]).unwrap()
    }

    pub async fn create_link<'env>(
        &self,
        base: AnyLinkableHash,
        target: AnyLinkableHash,
        link_type: impl Into<LinkType>,
        link_tag: LinkTag,
    ) -> ActionHash {
        let (ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = CreateLinkInput::new(
            base,
            target,
            link_type.into(),
            link_tag,
            ChainTopOrdering::default(),
        );
        let output = { host_fn::create_link::create_link(ribosome, call_context, input).unwrap() };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn delete_link<'env>(&self, link_add_hash: ActionHash) -> ActionHash {
        let (ribosome, call_context, workspace_lock) = self.unpack().await;
        let output = {
            host_fn::delete_link::delete_link(
                ribosome,
                call_context,
                DeleteLinkInput::new(link_add_hash, ChainTopOrdering::default()),
            )
            .unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn get_links<'env>(
        &self,
        base: AnyLinkableHash,
        type_query: LinkTypeRanges,
        link_tag: Option<LinkTag>,
        _options: GetLinksOptions,
    ) -> Vec<Link> {
        let (ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = GetLinksInput::new(base, type_query, link_tag);
        let output = {
            host_fn::get_links::get_links(ribosome, call_context, vec![input])
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn get_link_details<'env>(
        &self,
        base: AnyLinkableHash,
        type_query: LinkTypeRanges,
        tag: LinkTag,
        _options: GetLinksOptions,
    ) -> Vec<(SignedActionHashed, Vec<SignedActionHashed>)> {
        let (ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = GetLinksInput::new(base, type_query, Some(tag));
        let output = {
            host_fn::get_link_details::get_link_details(ribosome, call_context, vec![input])
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output.into()
    }

    pub async fn get_agent_activity(
        &self,
        agent: &AgentPubKey,
        query: &ChainQueryFilter,
        request: ActivityRequest,
    ) -> AgentActivity {
        let (ribosome, call_context, _) = self.unpack().await;
        let input = GetAgentActivityInput::new(agent.clone(), query.clone(), request);
        host_fn::get_agent_activity::get_agent_activity(ribosome, call_context, input).unwrap()
    }

    pub async fn call_zome_direct(&self, invocation: ZomeCallInvocation) -> ExternIO {
        let (ribosome, call_context, workspace_lock) = self.unpack().await;

        let output = {
            let host_access = call_context.host_context();
            let zcha = unwrap_to!(host_access => HostContext::ZomeCall).clone();
            ribosome.call_zome_function(zcha, invocation).unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();
        unwrap_to!(output => ZomeCallResponse::Ok).to_owned()
    }
}

#[macro_export]
macro_rules! test_entry_impl {
    ($type:ident) => {
        impl TryFrom<$type> for Entry {
            type Error = EntryError;
            fn try_from(n: $type) -> Result<Self, Self::Error> {
                Ok(Entry::App(SerializedBytes::try_from(n)?.try_into()?))
            }
        }

        impl TryFrom<Entry> for $type {
            type Error = SerializedBytesError;
            fn try_from(entry: Entry) -> Result<Self, Self::Error> {
                let entry = unwrap_to!(entry => Entry::App).clone();
                $type::try_from(entry.into_sb())
            }
        }
    };
}

test_entry_impl!(ThisWasmEntry);
test_entry_impl!(Post);
test_entry_impl!(Msg);
test_entry_impl!(MaybeLinkable);
