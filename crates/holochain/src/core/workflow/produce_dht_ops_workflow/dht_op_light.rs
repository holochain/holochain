use crate::core::state::chain_cas::ChainCasBuf;
use error::{DhtOpConvertError, DhtOpConvertResult};
use holo_hash::{AnyDhtHash, EntryHash, HeaderHash};
use holochain_keystore::Signature;
use holochain_types::{
    dht_op::DhtOp,
    header::{HeaderHashed, NewEntryHeader},
};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::header::{self, Header, IntendedFor};
use serde::{Deserialize, Serialize};

pub mod error;

use tracing::*;

#[cfg(test)]
mod tests;

/// A type for storing in databases that don't need the actual
/// data. Everything is a hash of the type except the signatures.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum DhtOpLight {
    StoreElement(HeaderHash, Option<EntryHash>),
    StoreEntry(HeaderHash, EntryHash),
    RegisterAgentActivity(HeaderHash),
    RegisterReplacedBy(HeaderHash, EntryHash),
    RegisterDeletedBy(HeaderHash),
    RegisterDeletedEntryHeader(HeaderHash),
    RegisterAddLink(HeaderHash),
    RegisterRemoveLink(HeaderHash),
}

#[instrument(skip(cas))]
/// Convert a [DhtOp] to a [DhtOpLight] and basis
pub async fn dht_op_to_light_basis(
    op: DhtOp,
    cas: &ChainCasBuf<'_>,
) -> DhtOpConvertResult<(DhtOpLight, AnyDhtHash)> {
    let basis = dht_basis(&op, &cas).await?;
    match op {
        DhtOp::StoreElement(_, h, _) => {
            let e = h.entry_data().map(|(e, _)| e.clone());
            let (_, h) = HeaderHashed::from_content(h).await.into();
            Ok((DhtOpLight::StoreElement(h, e), basis))
        }
        DhtOp::StoreEntry(_, h, _) => {
            let e = h.entry().clone();
            let (_, h) = HeaderHashed::from_content(h.into()).await.into();
            Ok((DhtOpLight::StoreEntry(h, e), basis))
        }
        DhtOp::RegisterAgentActivity(_, h) => {
            let (_, h) = HeaderHashed::from_content(h).await.into();
            Ok((DhtOpLight::RegisterAgentActivity(h), basis))
        }
        DhtOp::RegisterReplacedBy(_, h, _) => {
            let e = h.entry_hash.clone();
            let (_, h) = HeaderHashed::from_content(h.into()).await.into();
            Ok((DhtOpLight::RegisterReplacedBy(h, e), basis))
        }
        DhtOp::RegisterDeletedBy(_, h) => {
            let (_, h) = HeaderHashed::from_content(h.into()).await.into();
            Ok((DhtOpLight::RegisterDeletedBy(h), basis))
        }
        DhtOp::RegisterDeletedEntryHeader(_, h) => {
            let (_, h) = HeaderHashed::from_content(h.into()).await.into();
            Ok((DhtOpLight::RegisterDeletedEntryHeader(h), basis))
        }
        DhtOp::RegisterAddLink(_, h) => {
            let (_, h) = HeaderHashed::from_content(h.into()).await.into();
            Ok((DhtOpLight::RegisterAddLink(h), basis))
        }
        DhtOp::RegisterRemoveLink(_, h) => {
            let (_, h) = HeaderHashed::from_content(h.into()).await.into();
            Ok((DhtOpLight::RegisterRemoveLink(h), basis))
        }
    }
}

/// Convert a DhtOpLight into a DhtOp (render all the hashes to values)
/// This only checks the cas so can only be used with ops that you are an authority
// or author of.
pub async fn light_to_op(op: DhtOpLight, cas: &ChainCasBuf<'_>) -> DhtOpConvertResult<DhtOp> {
    let op_name = format!("{:?}", op);
    match op {
        DhtOpLight::StoreElement(h, _) => {
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
        DhtOpLight::StoreEntry(h, _) => {
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
        DhtOpLight::RegisterAgentActivity(h) => {
            let (header, sig) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner()
                .0
                .into_header_and_signature();
            Ok(DhtOp::RegisterAgentActivity(sig, header.into_content()))
        }
        DhtOpLight::RegisterReplacedBy(h, _) => {
            let (header, entry) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner();
            let (header, sig) = header.into_header_and_signature();
            let header = match header.into_content() {
                Header::EntryUpdate(u) => u,
                h => {
                    return Err(DhtOpConvertError::HeaderMismatch(
                        format!("{:?}", h),
                        op_name,
                    ));
                }
            };
            // Entry must be here because it's a RegisterReplacedBy
            // This is not true for private entries so we should only error
            // if this is meant to be public
            let entry = match header.entry_type.visibility() {
                EntryVisibility::Public => {
                    Some(entry.ok_or(DhtOpConvertError::MissingData)?.into())
                }
                EntryVisibility::Private => entry.map(Box::new),
            };
            Ok(DhtOp::RegisterReplacedBy(sig, header, entry))
        }
        DhtOpLight::RegisterDeletedBy(header_hash) => {
            let (header, sig) = get_element_delete(header_hash, op_name.clone(), &cas).await?;
            Ok(DhtOp::RegisterDeletedBy(sig, header))
        }
        DhtOpLight::RegisterDeletedEntryHeader(header_hash) => {
            let (header, sig) = get_element_delete(header_hash, op_name.clone(), &cas).await?;
            Ok(DhtOp::RegisterDeletedEntryHeader(sig, header))
        }
        DhtOpLight::RegisterAddLink(h) => {
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
        DhtOpLight::RegisterRemoveLink(h) => {
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
    cas: &ChainCasBuf<'_>,
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

#[instrument(skip(op, cas))]
/// Returns the basis hash which determines which agents will receive this DhtOp
pub async fn dht_basis(op: &DhtOp, cas: &ChainCasBuf<'_>) -> DhtOpConvertResult<AnyDhtHash> {
    Ok(match op {
        DhtOp::StoreElement(_, header, _) => {
            let (_, hash): (_, HeaderHash) =
                HeaderHashed::from_content(header.clone()).await.into();
            hash.into()
        }
        DhtOp::StoreEntry(_, header, _) => header.entry().clone().into(),
        DhtOp::RegisterAgentActivity(_, header) => header.author().clone().into(),
        DhtOp::RegisterReplacedBy(_, header, _) => match &header.intended_for {
            IntendedFor::Header => header.replaces_address.clone().into(),
            IntendedFor::Entry => get_entry_hash_for_header(&header.replaces_address, &cas)
                .await?
                .into(),
        },
        DhtOp::RegisterDeletedBy(_, header) => header.removes_address.clone().into(),
        DhtOp::RegisterDeletedEntryHeader(_, header) => {
            get_entry_hash_for_header(&header.removes_address, &cas)
                .await?
                .into()
        }
        DhtOp::RegisterAddLink(_, header) => header.base_address.clone().into(),
        DhtOp::RegisterRemoveLink(_, header) => header.base_address.clone().into(),
    })
}

#[instrument(skip(cas))]
async fn get_entry_hash_for_header(
    header_hash: &HeaderHash,
    cas: &ChainCasBuf<'_>,
) -> DhtOpConvertResult<EntryHash> {
    debug!(%header_hash);
    let entry = cas
        .get_header(header_hash)
        .await?
        .and_then(|e| e.header().entry_data().map(|(hash, _)| hash.clone()));
    entry.ok_or_else(|| DhtOpConvertError::MissingEntryDataForHeader(header_hash.clone()))
}
