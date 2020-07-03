use crate::core::ribosome::error::{RibosomeError, RibosomeResult};
use crate::core::{
    ribosome::{HostContext, RibosomeT},
    state::metadata::{LinkMetaKey, LinkMetaVal},
    workflow::InvokeZomeWorkspace,
};
use futures::future::FutureExt;
use holochain_state::error::DatabaseResult;
use holochain_zome_types::link::Link;
use holochain_zome_types::GetLinksInput;
use holochain_zome_types::GetLinksOutput;
use must_future::MustBoxFuture;
use std::convert::TryInto;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_links<'a>(
    ribosome: Arc<impl RibosomeT>,
    host_context: Arc<HostContext>,
    input: GetLinksInput,
) -> RibosomeResult<GetLinksOutput> {
    let (base_address, tag) = input.into_inner();

    let base_address = base_address.try_into()?;

    // Get zome id
    let zome_id: holochain_types::header::ZomeId = match ribosome
        .dna_file()
        .dna
        .zomes
        .iter()
        .position(|(name, _)| name == &host_context.zome_name)
    {
        Some(index) => holochain_types::header::ZomeId::from(index as u8),
        None => Err(RibosomeError::ZomeNotExists(host_context.zome_name.clone()))?,
    };

    let call =
        |workspace: &'a InvokeZomeWorkspace| -> MustBoxFuture<'a, DatabaseResult<Vec<LinkMetaVal>>> {
            async move {
                let cascade = workspace.cascade();

                // Create the key
                let key = match tag.as_ref() {
                    Some(tag) => LinkMetaKey::BaseZomeTag(&base_address, zome_id, tag),
                    None => LinkMetaKey::BaseZome(&base_address, zome_id),
                };

                // Get te links from the dht
                cascade
                    .dht_get_links(&key)
                    .await
            }
            .boxed()
            .into()
        };

    let links = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        unsafe { host_context.workspace.apply_ref(call).await }
    })??;

    let links: Vec<Link> = links.into_iter().map(|l| l.into_link()).collect();

    Ok(GetLinksOutput::new(links))
}
