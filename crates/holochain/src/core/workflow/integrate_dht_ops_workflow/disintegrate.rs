use super::*;
use holo_hash::EntryHash;

pub async fn disintegrate_single_metadata<C, P>(
    op: DhtOpLight,
    element_store: &ElementBuf<P>,
    meta_store: &mut C,
) -> DhtOpConvertResult<()>
where
    P: PrefixType,
    C: MetadataBufT<P>,
{
    match op {
        DhtOpLight::StoreElement(hash, _, _) => {
            meta_store.deregister_element_header(hash)?;
        }
        DhtOpLight::StoreEntry(hash, _, _) => {
            let new_entry_header = get_header(hash, element_store).await?.try_into()?;
            // Reference to headers
            meta_store.deregister_header(new_entry_header)?;
        }
        DhtOpLight::RegisterAgentActivity(hash, _) => {
            let header = get_header(hash, element_store).await?;
            // register agent activity on this agents pub key
            meta_store.deregister_activity(header)?;
        }
        DhtOpLight::RegisterUpdatedBy(hash, _, _) => {
            let header = get_header(hash, element_store).await?.try_into()?;
            meta_store.deregister_update(header)?;
        }
        DhtOpLight::RegisterDeletedEntryHeader(hash, _)
        | DhtOpLight::RegisterDeletedBy(hash, _) => {
            let header = get_header(hash, element_store).await?.try_into()?;
            meta_store.deregister_delete(header)?
        }
        DhtOpLight::RegisterAddLink(hash, _) => {
            let header = get_header(hash, element_store).await?.try_into()?;
            meta_store.deregister_add_link(header)?;
        }
        DhtOpLight::RegisterRemoveLink(hash, _) => {
            let header = get_header(hash, element_store).await?.try_into()?;
            meta_store.deregister_remove_link(header)?;
        }
    }
    Ok(())
}

/// DISABLED
/// TODO: figure out how to garbage collect ops without removing another ops data
pub fn disintegrate_single_data<P: PrefixType>(
    _op: DhtOpLight,
    _element_store: &mut ElementBuf<P>,
) {
}

/// Store a DhtOp's data in an element buf without dependency checks
pub fn _disintegrate_single_data<P: PrefixType>(op: DhtOpLight, element_store: &mut ElementBuf<P>) {
    match op {
        DhtOpLight::StoreElement(header, maybe_entry, _) => {
            delete_data(header, maybe_entry, element_store);
        }
        DhtOpLight::StoreEntry(new_entry_header, entry, _) => {
            delete_data(new_entry_header, Some(entry), element_store);
        }
        DhtOpLight::RegisterAgentActivity(header, _) => {
            delete_data(header, None, element_store);
        }
        DhtOpLight::RegisterUpdatedBy(entry_update, _, _) => {
            delete_data(entry_update, None, element_store);
        }
        DhtOpLight::RegisterDeletedEntryHeader(element_delete, _) => {
            delete_data(element_delete, None, element_store);
        }
        DhtOpLight::RegisterDeletedBy(element_delete, _) => {
            delete_data(element_delete, None, element_store);
        }
        DhtOpLight::RegisterAddLink(link_add, _) => {
            delete_data(link_add, None, element_store);
        }
        DhtOpLight::RegisterRemoveLink(link_remove, _) => {
            delete_data(link_remove, None, element_store);
        }
    }
}

fn delete_data<P: PrefixType>(
    header_hash: HeaderHash,
    entry_hash: Option<EntryHash>,
    element_store: &mut ElementBuf<P>,
) {
    element_store.delete(header_hash, entry_hash);
}
