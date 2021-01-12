use error::DhtOpConvertError;
use error::DhtOpConvertResult;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_state::element_buf::ElementBuf;
use holochain_types::prelude::*;

pub mod error;

use holochain_sqlite::prelude::PrefixType;
use tracing::*;

#[cfg(test)]
mod tests;

/// Convert a DhtOpLight into a DhtOp (render all the hashes to values)
/// This only checks the ElementVault so can only be used with ops that you are
/// an authority or author of.
pub fn light_to_op<P: PrefixType>(
    op: DhtOpLight,
    cas: &ElementBuf<P>,
) -> DhtOpConvertResult<DhtOp> {
    let op_name = format!("{:?}", op);
    match op {
        DhtOpLight::StoreElement(h, _, _) => {
            let (header, entry) = cas
                .get_element(&h)?
                .ok_or_else(|| DhtOpConvertError::MissingData(h.into()))?
                .into_inner();
            // TODO: Could use this signature? Is it the same?
            // Should we not be storing the signature in the DhtOpLight?
            let (header, sig) = header.into_header_and_signature();
            let entry = entry.into_option().map(Box::new);
            Ok(DhtOp::StoreElement(sig, header.into_content(), entry))
        }
        DhtOpLight::StoreEntry(h, _, _) => {
            let (header, entry) = cas
                .get_element(&h)?
                .ok_or_else(|| DhtOpConvertError::MissingData(h.into()))?
                .into_inner();
            let (header, sig) = header.into_header_and_signature();
            let header = match header.into_content() {
                Header::Create(c) => NewEntryHeader::Create(c),
                Header::Update(c) => NewEntryHeader::Update(c),
                _ => return Err(DhtOpConvertError::HeaderEntryMismatch),
            };

            let entry = match header.visibility() {
                // Entry must be here because it's a StoreEntry
                EntryVisibility::Public => entry
                    .into_option()
                    .ok_or_else(|| DhtOpConvertError::MissingData(header.entry().clone().into()))?,
                // If the entry is not here and you were meant to have access
                // it's because you were using a database without access to private entries
                // If not then you should handle this error
                EntryVisibility::Private => entry
                    .into_option()
                    .ok_or(DhtOpConvertError::StoreEntryOnPrivate)?,
            };
            Ok(DhtOp::StoreEntry(sig, header, Box::new(entry)))
        }
        DhtOpLight::RegisterAgentActivity(h, _) => {
            let (header, sig) = cas
                .get_header(&h)?
                .ok_or_else(|| DhtOpConvertError::MissingData(h.into()))?
                .into_header_and_signature();
            Ok(DhtOp::RegisterAgentActivity(sig, header.into_content()))
        }
        DhtOpLight::RegisterUpdatedContent(h, _, _) => {
            let (header, entry) = cas
                .get_element(&h)?
                .ok_or_else(|| DhtOpConvertError::MissingData(h.into()))?
                .into_inner();
            let (header, sig) = header.into_header_and_signature();
            let header = match header.into_content() {
                Header::Update(u) => u,
                h => {
                    return Err(DhtOpConvertError::HeaderMismatch(
                        format!("{:?}", h),
                        op_name,
                    ));
                }
            };
            let entry = entry.into_option().map(Box::new);
            Ok(DhtOp::RegisterUpdatedContent(sig, header, entry))
        }
        DhtOpLight::RegisterUpdatedElement(h, _, _) => {
            let (header, entry) = cas
                .get_element(&h)?
                .ok_or_else(|| DhtOpConvertError::MissingData(h.into()))?
                .into_inner();
            let (header, sig) = header.into_header_and_signature();
            let header = match header.into_content() {
                Header::Update(u) => u,
                h => {
                    return Err(DhtOpConvertError::HeaderMismatch(
                        format!("{:?}", h),
                        op_name,
                    ));
                }
            };
            let entry = entry.into_option().map(Box::new);
            Ok(DhtOp::RegisterUpdatedElement(sig, header, entry))
        }
        DhtOpLight::RegisterDeletedBy(header_hash, _) => {
            let (header, sig) = get_element_delete(header_hash, op_name, &cas)?;
            Ok(DhtOp::RegisterDeletedBy(sig, header))
        }
        DhtOpLight::RegisterDeletedEntryHeader(header_hash, _) => {
            let (header, sig) = get_element_delete(header_hash, op_name, &cas)?;
            Ok(DhtOp::RegisterDeletedEntryHeader(sig, header))
        }
        DhtOpLight::RegisterAddLink(h, _) => {
            let (header, sig) = cas
                .get_element(&h)?
                .ok_or_else(|| DhtOpConvertError::MissingData(h.into()))?
                .into_inner()
                .0
                .into_header_and_signature();
            let header = match header.into_content() {
                Header::CreateLink(u) => u,
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
                .get_element(&h)?
                .ok_or_else(|| DhtOpConvertError::MissingData(h.into()))?
                .into_inner()
                .0
                .into_header_and_signature();
            let header = match header.into_content() {
                Header::DeleteLink(u) => u,
                h => {
                    return Err(DhtOpConvertError::HeaderMismatch(
                        format!("{:?}", h),
                        op_name,
                    ));
                }
            };
            Ok(DhtOp::RegisterRemoveLink(sig, header))
        }
    }
}

fn get_element_delete<P: PrefixType>(
    header_hash: HeaderHash,
    op_name: String,
    cas: &ElementBuf<P>,
) -> DhtOpConvertResult<(header::Delete, Signature)> {
    let (header, sig) = cas
        .get_header(&header_hash)?
        .ok_or_else(|| DhtOpConvertError::MissingData(header_hash.into()))?
        .into_header_and_signature();
    match header.into_content() {
        Header::Delete(u) => Ok((u, sig)),
        h => Err(DhtOpConvertError::HeaderMismatch(
            format!("{:?}", h),
            op_name,
        )),
    }
}

#[instrument(skip(cas))]
async fn get_entry_hash_for_header(
    header_hash: &HeaderHash,
    cas: &ElementBuf,
) -> DhtOpConvertResult<EntryHash> {
    debug!(%header_hash);
    let entry = cas
        .get_header(header_hash)?
        .and_then(|e| e.header().entry_data().map(|(hash, _)| hash.clone()));
    entry.ok_or_else(|| DhtOpConvertError::MissingEntryDataForHeader(header_hash.clone()))
}
