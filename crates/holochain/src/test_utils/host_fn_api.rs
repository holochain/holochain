use crate::{
    conductor::ConductorHandle,
    core::{
        ribosome::{host_fn, wasm_ribosome::WasmRibosome, CallContext, ZomeCallHostAccess},
        state::{metadata::LinkMetaKey, workspace::Workspace},
        workflow::{unsafe_call_zome_workspace::UnsafeCallZomeWorkspace, CallZomeWorkspace},
    },
};
use hdk3::prelude::EntryError;
use holo_hash::{AnyDhtHash, EntryHash, HeaderHash};
use holochain_keystore::KeystoreSender;
use holochain_p2p::{
    actor::{GetLinksOptions, GetOptions, HolochainP2pRefToCell},
    HolochainP2pCell,
};
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    env::{EnvironmentRead, EnvironmentWrite},
    prelude::{GetDb, WriteManager},
};
use holochain_types::{cell::CellId, dna::DnaFile, element::Element, Entry};
use holochain_zome_types::{
    entry_def,
    header::*,
    link::{Link, LinkTag},
    metadata::Details,
    zome::ZomeName,
    CommitEntryInput, DeleteEntryInput, GetDetailsInput, GetInput, GetLinksInput, LinkEntriesInput,
    RemoveLinkInput, UpdateEntryInput,
};
use std::sync::Arc;
use tracing::*;
use unwrap_to::unwrap_to;

// Commit entry types //
// Useful for when you want to commit something
// that will match entry defs
pub const POST_ID: &str = "post";

#[derive(
    Default, Debug, PartialEq, Clone, SerializedBytes, serde::Serialize, serde::Deserialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Post(pub String);

#[derive(Clone)]
pub struct CallData {
    pub ribosome: WasmRibosome,
    pub zome_name: ZomeName,
    pub network: HolochainP2pCell,
    pub keystore: KeystoreSender,
}

impl CallData {
    pub async fn create(
        cell_id: &CellId,
        handle: &ConductorHandle,
        dna_file: &DnaFile,
    ) -> (EnvironmentWrite, CallData) {
        let env = handle.get_cell_env(cell_id).await.unwrap();
        let keystore = env.keystore().clone();
        let network = handle
            .holochain_p2p()
            .to_cell(cell_id.dna_hash().clone(), cell_id.agent_pubkey().clone());

        let zome_name = dna_file.dna().zomes.get(0).unwrap().0.clone();
        let ribosome = WasmRibosome::new(dna_file.clone());
        let call_data = CallData {
            ribosome,
            zome_name,
            network,
            keystore,
        };
        (env, call_data)
    }
}

pub async fn commit_entry<'env, E: Into<entry_def::EntryDefId>>(
    env: &EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    entry: Entry,
    entry_def_id: E,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;

    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs).unwrap();

    let input = CommitEntryInput::new((entry_def_id.into(), entry));

    let output = {
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::commit_entry::commit_entry(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

pub async fn delete_entry<'env>(
    env: &EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    hash: HeaderHash,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;

    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs).unwrap();

    let input = DeleteEntryInput::new(hash);

    let output = {
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        let r = host_fn::delete_entry::delete_entry(ribosome.clone(), call_context.clone(), input);
        let r = r.map_err(|e| {
            debug!(%e);
            e
        });
        r.unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

pub async fn update_entry<'env, E: Into<entry_def::EntryDefId>>(
    env: &EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    entry: Entry,
    entry_def_id: E,
    original_header_hash: HeaderHash,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;

    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs).unwrap();

    let input = UpdateEntryInput::new((entry_def_id.into(), entry, original_header_hash));

    let output = {
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::update_entry::update_entry(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

pub async fn get(
    env: &EnvironmentRead,
    dbs: &impl GetDb,
    call_data: CallData,
    entry_hash: AnyDhtHash,
    _options: GetOptions,
) -> Option<Element> {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs).unwrap();

    let input = GetInput::new((
        entry_hash.clone().into(),
        holochain_zome_types::entry::GetOptions,
    ));

    let output = {
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::get::get(ribosome.clone(), call_context.clone(), input).unwrap()
    };
    output.into_inner()
}

pub async fn get_details<'env>(
    env: &EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    entry_hash: AnyDhtHash,
    _options: GetOptions,
) -> Option<Details> {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;

    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs).unwrap();

    let input = GetDetailsInput::new((
        entry_hash.clone().into(),
        holochain_zome_types::entry::GetOptions,
    ));

    let output = {
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::get_details::get_details(ribosome.clone(), call_context.clone(), input).unwrap()
    };
    output.into_inner()
}

pub async fn link_entries<'env>(
    env: &EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    base: EntryHash,
    target: EntryHash,
    link_tag: LinkTag,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;

    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs).unwrap();

    let input = LinkEntriesInput::new((base.clone(), target.clone(), link_tag));

    let output = {
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::link_entries::link_entries(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

pub async fn remove_link<'env>(
    env: &EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    link_add_hash: HeaderHash,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;

    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs).unwrap();

    let input = RemoveLinkInput::new(link_add_hash);

    let output = {
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::remove_link::remove_link(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

pub async fn get_links<'env>(
    env: &EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    base: EntryHash,
    link_tag: Option<LinkTag>,
    _options: GetLinksOptions,
) -> Vec<Link> {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;

    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs).unwrap();

    let input = GetLinksInput::new((base.clone(), link_tag));

    let output = {
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::get_links::get_links(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner().into()
}

pub async fn get_link_details<'env>(
    env: &EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    base: EntryHash,
    tag: LinkTag,
    options: GetLinksOptions,
) -> Vec<(LinkAdd, Vec<LinkRemove>)> {
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs).unwrap();

    let mut cascade = workspace.cascade(call_data.network);
    let key = LinkMetaKey::BaseZomeTag(&base, 0.into(), &tag);
    cascade.get_link_details(&key, options).await.unwrap()
}

impl TryFrom<Post> for Entry {
    type Error = EntryError;
    fn try_from(post: Post) -> Result<Self, Self::Error> {
        Ok(Entry::App(SerializedBytes::try_from(post)?.try_into()?))
    }
}

impl TryFrom<Entry> for Post {
    type Error = SerializedBytesError;
    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
        let entry = unwrap_to!(entry => Entry::App).clone();
        Ok(Post::try_from(entry.into_sb())?)
    }
}
