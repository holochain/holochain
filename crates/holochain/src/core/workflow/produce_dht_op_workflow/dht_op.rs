use crate::core::state::{cascade::Cascade, metadata::MetadataBufT};
use error::{DhtOpConvertError, DhtOpConvertResult};
use header::UpdateBasis;
use holo_hash::HeaderHash;
use holochain_types::{
    composite_hash::{AnyDhtHash, EntryHash},
    dht_op::{DhtOp, DhtOpHashes, DhtOpLight},
    header,
};

pub mod error;

#[cfg(test)]
mod tests;

/// Convert a [DhtOp] to a [DhtOpHashes]
pub async fn dht_op_into_light(op: DhtOp, cascade: Cascade<'_>) -> DhtOpConvertResult<DhtOpLight> {
    let basis = dht_basis(&op, &cascade).await?;
    match op {
        DhtOp::StoreElement(s, h, _) => {
            let e = h.entry_data().map(|(e, _)| e.clone());
            let (_, h) = header::HeaderHashed::with_data(h).await?.into();
            Ok(DhtOpLight {
                basis,
                op: DhtOpHashes::StoreElement(s, h, e),
            })
        }
        DhtOp::StoreEntry(s, h, _) => {
            let e = h.entry().clone();
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok(DhtOpLight {
                basis,
                op: DhtOpHashes::StoreEntry(s, h, e),
            })
        }
        DhtOp::RegisterAgentActivity(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h).await?.into();
            Ok(DhtOpLight {
                basis,
                op: DhtOpHashes::RegisterAgentActivity(s, h),
            })
        }
        DhtOp::RegisterReplacedBy(s, h, _) => {
            let e = h.entry_hash.clone();
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok(DhtOpLight {
                basis,
                op: DhtOpHashes::RegisterReplacedBy(s, h, e),
            })
        }
        DhtOp::RegisterDeletedBy(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok(DhtOpLight {
                basis,
                op: DhtOpHashes::RegisterAgentActivity(s, h),
            })
        }
        DhtOp::RegisterAddLink(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok(DhtOpLight {
                basis,
                op: DhtOpHashes::RegisterAgentActivity(s, h),
            })
        }
        DhtOp::RegisterRemoveLink(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok(DhtOpLight {
                basis,
                op: DhtOpHashes::RegisterAgentActivity(s, h),
            })
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
