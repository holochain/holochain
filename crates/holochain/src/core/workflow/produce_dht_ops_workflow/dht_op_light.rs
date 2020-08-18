use crate::core::state::element_buf::ElementBuf;
use error::{DhtOpConvertError, DhtOpConvertResult};
use holo_hash::{EntryHash, HeaderHash};
use holochain_keystore::Signature;
use holochain_types::{
    dht_op::{DhtOp, DhtOpLight},
    header::NewEntryHeader,
};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::header::{self, Header};

pub mod error;

use tracing::*;

#[cfg(test)]
mod tests;

/// Convert a DhtOpLight into a DhtOp (render all the hashes to values)
/// This only checks the ElementVault so can only be used with ops that you are
/// an authority or author of.
pub async fn light_to_op(op: DhtOpLight, cas: &ElementBuf<'_>) -> DhtOpConvertResult<DhtOp> {
    let op_name = format!("{:?}", op);
    match op {
        DhtOpLight::StoreElement(h, _, _) => {
            let (header, entry) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner();
            // TODO: Could use this signature? Is it the same?
            // Should we not be storing the signature in the DhtOpLight?
            let (header, sig) = header.into_header_and_signature();
            let entry = entry.map(Box::new);
            Ok(DhtOp::StoreElement(sig, header.into_content(), entry))
        }
        DhtOpLight::StoreEntry(h, _, _) => {
            let (header, entry) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner();
            let (header, sig) = header.into_header_and_signature();
            let header = match header.into_content() {
                Header::EntryCreate(c) => NewEntryHeader::Create(c),
                Header::EntryUpdate(c) => NewEntryHeader::Update(c),
                _ => return Err(DhtOpConvertError::HeaderEntryMismatch),
            };

            let entry = match header.visibility() {
                // Entry must be here because it's a StoreEntry
                EntryVisibility::Public => entry.ok_or(DhtOpConvertError::MissingData)?,
                // If the entry is not here and you were meant to have access
                // it's because you were using a database without access to private entries
                // If not then you should handle this error
                EntryVisibility::Private => entry.ok_or(DhtOpConvertError::StoreEntryOnPrivate)?,
            };
            Ok(DhtOp::StoreEntry(sig, header, Box::new(entry)))
        }
        DhtOpLight::RegisterAgentActivity(h, _) => {
            let (header, sig) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner()
                .0
                .into_header_and_signature();
            Ok(DhtOp::RegisterAgentActivity(sig, header.into_content()))
        }
        DhtOpLight::RegisterUpdatedBy(h, _, _) => {
            let (header, sig) = cas
                .get_header(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_header_and_signature();
            let header = match header.into_content() {
                Header::EntryUpdate(u) => u,
                h => {
                    return Err(DhtOpConvertError::HeaderMismatch(
                        format!("{:?}", h),
                        op_name,
                    ));
                }
            };
            Ok(DhtOp::RegisterUpdatedBy(sig, header))
        }
        DhtOpLight::RegisterDeletedBy(header_hash, _) => {
            let (header, sig) = get_element_delete(header_hash, op_name.clone(), &cas).await?;
            Ok(DhtOp::RegisterDeletedBy(sig, header))
        }
        DhtOpLight::RegisterDeletedEntryHeader(header_hash, _) => {
            let (header, sig) = get_element_delete(header_hash, op_name.clone(), &cas).await?;
            Ok(DhtOp::RegisterDeletedEntryHeader(sig, header))
        }
        DhtOpLight::RegisterAddLink(h, _) => {
            let (header, sig) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner()
                .0
                .into_header_and_signature();
            let header = match header.into_content() {
                Header::LinkAdd(u) => u,
                h => {
                    return Err(DhtOpConvertError::HeaderMismatch(
                        format!("{:?}", h),
                        op_name,
                    ));
                }
            };
            Ok(DhtOp::RegisterAddLink(sig, header))
        }
        DhtOpLight::RegisterRemoveLink(h, _) => {
            let (header, sig) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner()
                .0
                .into_header_and_signature();
            let header = match header.into_content() {
                Header::LinkRemove(u) => u,
                h => {
                    return Err(DhtOpConvertError::HeaderMismatch(
                        format!("{:?}", h),
                        op_name,
                    ))
                }
            };
            Ok(DhtOp::RegisterRemoveLink(sig, header))
        }
    }
}

async fn get_element_delete(
    header_hash: HeaderHash,
    op_name: String,
    cas: &ElementBuf<'_>,
) -> DhtOpConvertResult<(header::ElementDelete, Signature)> {
    let (header, sig) = cas
        .get_element(&header_hash)
        .await?
        .ok_or(DhtOpConvertError::MissingData)?
        .into_inner()
        .0
        .into_header_and_signature();
    match header.into_content() {
        Header::ElementDelete(u) => Ok((u, sig)),
        h => Err(DhtOpConvertError::HeaderMismatch(
            format!("{:?}", h),
            op_name,
        )),
    }
}

#[instrument(skip(cas))]
async fn get_entry_hash_for_header(
    header_hash: &HeaderHash,
    cas: &ElementBuf<'_>,
) -> DhtOpConvertResult<EntryHash> {
    debug!(%header_hash);
    let entry = cas
        .get_header(header_hash)
        .await?
        .and_then(|e| e.header().entry_data().map(|(hash, _)| hash.clone()));
    entry.ok_or_else(|| DhtOpConvertError::MissingEntryDataForHeader(header_hash.clone()))
}
