use crate::core::state::{cascade::Cascade, metadata::MetadataBufT};
use error::{DhtOpConvertError, DhtOpConvertResult};
use header::UpdateBasis;
use holo_hash::HeaderHash;
use holochain_keystore::Signature;
use holochain_types::{
    composite_hash::{AnyDhtHash, EntryHash},
    dht_op::DhtOp,
    header,
};
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

/// Convert a [DhtOp] to a [DhtOpLight]
pub async fn dht_op_to_light_basis(
    op: DhtOp,
    cascade: Cascade<'_>,
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

// TODO: Remove when used
#[allow(dead_code)]
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
