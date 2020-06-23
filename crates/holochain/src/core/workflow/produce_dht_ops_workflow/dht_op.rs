use crate::core::state::{cascade::Cascade, chain_cas::ChainCasBuf, metadata::MetadataBufT};
use error::{DhtOpConvertError, DhtOpConvertResult};
use header::{NewEntryHeader, UpdateBasis};
use holo_hash::{Hashed, HeaderHash};
use holochain_keystore::Signature;
use holochain_types::{
    composite_hash::{AnyDhtHash, EntryHash},
    dht_op::DhtOp,
    header, Header,
};
use holochain_zome_types::entry_def::EntryVisibility;
use serde::{Deserialize, Serialize};

pub mod error;

#[cfg(test)]
mod tests;

/// A type for storing in databases that don't need the actual
/// data. Everything is a hash of the type except the signatures.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DhtOpLight {
    StoreElement(Signature, HeaderHash, Option<EntryHash>),
    StoreEntry(Signature, HeaderHash, EntryHash),
    RegisterAgentActivity(Signature, HeaderHash),
    RegisterReplacedBy(Signature, HeaderHash, EntryHash),
    RegisterDeletedBy(Signature, HeaderHash),
    RegisterAddLink(Signature, HeaderHash),
    RegisterRemoveLink(Signature, HeaderHash),
}

/// Convert a [DhtOp] to a [DhtOpLight] and basis
pub async fn dht_op_to_light_basis<C: MetadataBufT>(
    op: DhtOp,
    cascade: &Cascade<'_, C>,
) -> DhtOpConvertResult<(DhtOpLight, AnyDhtHash)> {
    let basis = dht_basis(&op, &cascade).await?;
    match op {
        DhtOp::StoreElement(s, h, _) => {
            let e = h.entry_data().map(|(e, _)| e.clone());
            let (_, h) = header::HeaderHashed::with_data(h).await?.into();
            Ok((DhtOpLight::StoreElement(s, h, e), basis))
        }
        DhtOp::StoreEntry(s, h, _) => {
            let e = h.entry().clone();
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok((DhtOpLight::StoreEntry(s, h, e), basis))
        }
        DhtOp::RegisterAgentActivity(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h).await?.into();
            Ok((DhtOpLight::RegisterAgentActivity(s, h), basis))
        }
        DhtOp::RegisterReplacedBy(s, h, _) => {
            let e = h.entry_hash.clone();
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok((DhtOpLight::RegisterReplacedBy(s, h, e), basis))
        }
        DhtOp::RegisterDeletedBy(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok((DhtOpLight::RegisterAgentActivity(s, h), basis))
        }
        DhtOp::RegisterAddLink(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok((DhtOpLight::RegisterAgentActivity(s, h), basis))
        }
        DhtOp::RegisterRemoveLink(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok((DhtOpLight::RegisterAgentActivity(s, h), basis))
        }
    }
}

/// Convert a DhtOpLight into a DhtOp (render all the hashes to values)
/// This only checks the cas so can only be used with ops that you are an authority
// or author of.
pub async fn light_to_op(op: DhtOpLight, cas: &ChainCasBuf<'_>) -> DhtOpConvertResult<DhtOp> {
    let op_name = format!("{:?}", op);
    match op {
        DhtOpLight::StoreElement(sig, h, _) => {
            let (header, entry) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner();
            // TODO: Could use this signature? Is it the same?
            // Should we not be storing the signature in the DhtOpLight?
            let (header, _sig) = header.into_header_and_signature();
            let entry = entry.map(Box::new);
            Ok(DhtOp::StoreElement(sig, header.into_content(), entry))
        }
        DhtOpLight::StoreEntry(sig, h, _) => {
            let (header, entry) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner();
            let (header, _sig) = header.into_header_and_signature();
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
        DhtOpLight::RegisterAgentActivity(sig, h) => {
            let (header, _sig) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner()
                .0
                .into_header_and_signature();
            Ok(DhtOp::RegisterAgentActivity(sig, header.into_content()))
        }
        DhtOpLight::RegisterReplacedBy(sig, h, _) => {
            let (header, entry) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner();
            let (header, _sig) = header.into_header_and_signature();
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
        DhtOpLight::RegisterDeletedBy(sig, h) => {
            let (header, _sig) = cas
                .get_element(&h)
                .await?
                .ok_or(DhtOpConvertError::MissingData)?
                .into_inner()
                .0
                .into_header_and_signature();
            let header = match header.into_content() {
                Header::EntryDelete(u) => u,
                h => {
                    return Err(DhtOpConvertError::HeaderMismatch(
                        format!("{:?}", h),
                        op_name,
                    ));
                }
            };
            Ok(DhtOp::RegisterDeletedBy(sig, header))
        }
        DhtOpLight::RegisterAddLink(sig, h) => {
            let (header, _sig) = cas
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
        DhtOpLight::RegisterRemoveLink(sig, h) => {
            let (header, _sig) = cas
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

/// Returns the basis hash which determines which agents will receive this DhtOp
pub async fn dht_basis<M: MetadataBufT>(
    op: &DhtOp,
    cascade: &Cascade<'_, M>,
) -> DhtOpConvertResult<AnyDhtHash> {
    Ok(match op {
        DhtOp::StoreElement(_, header, _) => {
            let (_, hash): (_, HeaderHash) = header::HeaderHashed::with_data(header.clone())
                .await?
                .into();
            hash.into()
        }
        DhtOp::StoreEntry(_, header, _) => header.entry().clone().into(),
        DhtOp::RegisterAgentActivity(_, header) => header.author().clone().into(),
        DhtOp::RegisterReplacedBy(_, header, _) => match &header.update_basis {
            UpdateBasis::Header => header.replaces_address.clone().into(),
            UpdateBasis::Entry => get_entry_hash_for_header(&header.replaces_address, &cascade)
                .await?
                .into(),
        },
        DhtOp::RegisterDeletedBy(_, header) => header.removes_address.clone().into(),
        DhtOp::RegisterAddLink(_, header) => header.base_address.clone().into(),
        DhtOp::RegisterRemoveLink(_, header) => header.base_address.clone().into(),
    })
}

async fn get_entry_hash_for_header<M: MetadataBufT>(
    header_hash: &HeaderHash,
    cascade: &Cascade<'_, M>,
) -> DhtOpConvertResult<EntryHash> {
    let entry = match cascade.dht_get_header_raw(header_hash).await? {
        Some(header) => header.header().entry_data().map(|(hash, _)| hash.clone()),
        None => todo!("try getting from the network"),
    };
    entry.ok_or(DhtOpConvertError::MissingEntry)
}
