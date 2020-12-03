#![allow(missing_docs)]

use crate::{
    conductor::ConductorHandle,
    conductor::{
        api::{CellConductorApi, CellConductorApiT, CellConductorReadHandle},
        interface::SignalBroadcaster,
    },
    core::ribosome::RibosomeT,
    core::ribosome::ZomeCallInvocation,
    core::{
        ribosome::{
            host_fn, wasm_ribosome::WasmRibosome, CallContext, HostAccess, ZomeCallHostAccess,
        },
        state::{metadata::LinkMetaKey, workspace::Workspace},
        workflow::{CallZomeWorkspace, CallZomeWorkspaceLock},
    },
};
use hdk3::prelude::EntryError;
use holo_hash::{AgentPubKey, AnyDhtHash, EntryHash, HeaderHash};
use holochain_keystore::KeystoreSender;
use holochain_p2p::{
    actor::{GetLinksOptions, GetOptions, HolochainP2pRefToCell},
    HolochainP2pCell,
};
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    env::EnvironmentWrite,
    prelude::{GetDb, WriteManager},
};
use holochain_types::{cell::CellId, dna::DnaFile, element::Element, Entry};
use holochain_zome_types::{
    element::SignedHeaderHashed,
    entry_def,
    link::{Link, LinkTag},
    metadata::Details,
    query::ActivityRequest,
    query::AgentActivity,
    query::ChainQueryFilter,
    zome::ZomeName,
    CreateInput, CreateLinkInput, DeleteInput, DeleteLinkInput, GetAgentActivityInput,
    GetDetailsInput, GetInput, GetLinksInput, UpdateInput, ZomeCallResponse,
};
use std::sync::Arc;
use tracing::*;
use unwrap_to::unwrap_to;

// Commit entry types //
// Useful for when you want to commit something
// that will match entry defs
pub const POST_ID: &str = "post";
pub const MSG_ID: &str = "msg";
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
    pub env: EnvironmentWrite,
    pub ribosome: WasmRibosome,
    pub zome_path: ZomePath,
    pub network: HolochainP2pCell,
    pub keystore: KeystoreSender,
    pub signal_tx: SignalBroadcaster,
    pub call_zome_handle: CellConductorReadHandle,
}

impl HostFnCaller {
    /// Create HostFnCaller for the first zome.
    #[deprecated = "use create_for_zome"]
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
        let env = handle.get_cell_env(cell_id).await.unwrap();
        let keystore = env.keystore().clone();
        let network = handle
            .holochain_p2p()
            .to_cell(cell_id.dna_hash().clone(), cell_id.agent_pubkey().clone());

        let zome_path = (
            cell_id.clone(),
            dna_file.dna().zomes.get(zome_index).unwrap().0.clone(),
        )
            .into();
        let ribosome = WasmRibosome::new(dna_file.clone());
        let signal_tx = handle.signal_broadcaster().await;
        let call_zome_handle =
            CellConductorApi::new(handle.clone(), cell_id.clone()).into_call_zome_handle();
        HostFnCaller {
            env,
            ribosome,
            zome_path,
            network,
            keystore,
            signal_tx,
            call_zome_handle,
        }
    }

    pub fn env(&self) -> EnvironmentWrite {
        self.env.clone()
    }

    pub fn explode(
        &self,
    ) -> (
        EnvironmentWrite,
        Arc<WasmRibosome>,
        Arc<CallContext>,
        CallZomeWorkspaceLock,
    ) {
        let HostFnCaller {
            env,
            network,
            keystore,
            ribosome,
            signal_tx,
            zome_path,
            call_zome_handle,
        } = self.clone();

        let (cell_id, zome_name) = zome_path.into();

        let workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();
        let workspace_lock = CallZomeWorkspaceLock::new(workspace);
        let host_access = ZomeCallHostAccess::new(
            workspace_lock.clone(),
            keystore,
            network,
            signal_tx,
            call_zome_handle,
            cell_id,
        );
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(CallContext::new(zome_name, host_access.into()));
        (env, ribosome, call_context, workspace_lock)
    }
}

impl HostFnCaller {
    pub async fn commit_entry<E: Into<entry_def::EntryDefId>>(
        &self,
        entry: Entry,
        entry_def_id: E,
    ) -> HeaderHash {
        let (env, ribosome, call_context, workspace_lock) = self.explode();
        let input = CreateInput::new((entry_def_id.into(), entry));
        let output = host_fn::create::create(ribosome, call_context, input).unwrap();

        // Write
        let mut guard = workspace_lock.write().await;
        let workspace = &mut guard;
        env.with_commit(|writer| workspace.flush_to_txn_ref(writer))
            .unwrap();

        output.into_inner()
    }

    pub async fn delete_entry<'env>(&self, hash: HeaderHash) -> HeaderHash {
        let (env, ribosome, call_context, workspace_lock) = self.explode();
        let input = DeleteInput::new(hash);
        let output = {
            let r = host_fn::delete::delete(ribosome, call_context, input);
            let r = r.map_err(|e| {
                debug!(%e);
                e
            });
            r.unwrap()
        };

        // Write
        let mut guard = workspace_lock.write().await;
        let workspace = &mut guard;
        env.with_commit(|writer| workspace.flush_to_txn_ref(writer))
            .unwrap();

        output.into_inner()
    }

    pub async fn update_entry<'env, E: Into<entry_def::EntryDefId>>(
        &self,
        entry: Entry,
        entry_def_id: E,
        original_header_hash: HeaderHash,
    ) -> HeaderHash {
        let (env, ribosome, call_context, workspace_lock) = self.explode();
        let input = UpdateInput::new((entry_def_id.into(), entry, original_header_hash));
        let output = { host_fn::update::update(ribosome, call_context, input).unwrap() };

        // Write
        let mut guard = workspace_lock.write().await;
        let workspace = &mut guard;
        env.with_commit(|writer| workspace.flush_to_txn_ref(writer))
            .unwrap();

        output.into_inner()
    }

    pub async fn get(&self, entry_hash: AnyDhtHash, _options: GetOptions) -> Option<Element> {
        let (_, ribosome, call_context, _) = self.explode();
        let input = GetInput::new((entry_hash, holochain_zome_types::entry::GetOptions));
        let output = { host_fn::get::get(ribosome, call_context, input).unwrap() };
        output.into_inner()
    }

    pub async fn get_details<'env>(
        &self,
        entry_hash: AnyDhtHash,
        _options: GetOptions,
    ) -> Option<Details> {
        let (_, ribosome, call_context, _) = self.explode();
        let input = GetDetailsInput::new((entry_hash, holochain_zome_types::entry::GetOptions));
        let output = { host_fn::get_details::get_details(ribosome, call_context, input).unwrap() };
        output.into_inner()
    }

    pub async fn create_link<'env>(
        &self,
        base: EntryHash,
        target: EntryHash,
        link_tag: LinkTag,
    ) -> HeaderHash {
        let (env, ribosome, call_context, workspace_lock) = self.explode();
        let input = CreateLinkInput::new((base.clone(), target.clone(), link_tag));
        let output = { host_fn::create_link::create_link(ribosome, call_context, input).unwrap() };

        // Write
        let mut guard = workspace_lock.write().await;
        let workspace = &mut guard;
        env.with_commit(|writer| workspace.flush_to_txn_ref(writer))
            .unwrap();

        output.into_inner()
    }

    pub async fn delete_link<'env>(&self, link_add_hash: HeaderHash) -> HeaderHash {
        let (env, ribosome, call_context, workspace_lock) = self.explode();
        let input = DeleteLinkInput::new(link_add_hash);
        let output = { host_fn::delete_link::delete_link(ribosome, call_context, input).unwrap() };

        // Write
        let mut guard = workspace_lock.write().await;
        let workspace = &mut guard;
        env.with_commit(|writer| workspace.flush_to_txn_ref(writer))
            .unwrap();

        output.into_inner()
    }

    pub async fn get_links<'env>(
        &self,
        base: EntryHash,
        link_tag: Option<LinkTag>,
        _options: GetLinksOptions,
    ) -> Vec<Link> {
        let (env, ribosome, call_context, workspace_lock) = self.explode();
        let input = GetLinksInput::new((base.clone(), link_tag));
        let output = { host_fn::get_links::get_links(ribosome, call_context, input).unwrap() };

        // Write
        let mut guard = workspace_lock.write().await;
        let workspace = &mut guard;
        env.with_commit(|writer| workspace.flush_to_txn_ref(writer))
            .unwrap();

        output.into_inner().into()
    }

    pub async fn get_link_details<'env>(
        &self,
        base: EntryHash,
        tag: LinkTag,
        options: GetLinksOptions,
    ) -> Vec<(SignedHeaderHashed, Vec<SignedHeaderHashed>)> {
        let mut workspace = CallZomeWorkspace::new(self.env.clone().into()).unwrap();
        let mut cascade = workspace.cascade(self.network.clone());
        let key = LinkMetaKey::BaseZomeTag(&base, 0.into(), &tag);
        cascade.get_link_details(&key, options).await.unwrap()
    }

    pub async fn get_agent_activity(
        &self,
        agent: &AgentPubKey,
        query: &ChainQueryFilter,
        request: ActivityRequest,
    ) -> AgentActivity {
        let (_, ribosome, call_context, _) = self.explode();
        let input = GetAgentActivityInput::new((agent.clone(), query.clone(), request));
        let output = {
            host_fn::get_agent_activity::get_agent_activity(ribosome, call_context, input).unwrap()
        };
        output.into_inner()
    }

    pub async fn call_zome_direct(&self, invocation: ZomeCallInvocation) -> SerializedBytes {
        let (env, ribosome, call_context, workspace_lock) = self.explode();

        let output = {
            let host_access = call_context.host_access();
            let zcha = unwrap_to!(host_access => HostAccess::ZomeCall).clone();
            ribosome.call_zome_function(zcha, invocation).unwrap()
        };

        // Write
        let mut guard = workspace_lock.write().await;
        let workspace = &mut guard;
        env.with_commit(|writer| workspace.flush_to_txn_ref(writer))
            .unwrap();
        let output = unwrap_to!(output => ZomeCallResponse::Ok).clone();

        output.into_inner()
    }
}

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
                Ok($type::try_from(entry.into_sb())?)
            }
        }
    };
}

test_entry_impl!(ThisWasmEntry);
test_entry_impl!(Post);
test_entry_impl!(Msg);
test_entry_impl!(MaybeLinkable);
