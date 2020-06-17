use crate::core::state::cascade::Cascade;
use error::{DhtOpConvertError, DhtOpConvertResult};
use header::UpdateBasis;
use holochain_types::{
    dht_op::{DhtOp, DhtOpLight, RegisterReplacedByLight},
    header,
};

pub mod error;

/// Convert a [DhtOp] to a [DhtOpLight]
pub async fn dht_op_into_light(op: DhtOp, cascade: Cascade<'_>) -> DhtOpConvertResult<DhtOpLight> {
    match op {
        DhtOp::StoreElement(s, h, _) => {
            let e = h.entry_data().map(|(e, _)| e.clone());
            let (_, h) = header::HeaderHashed::with_data(h).await?.into();
            Ok(DhtOpLight::StoreElement(s, h, e))
        }
        DhtOp::StoreEntry(s, h, _) => {
            let e = h.entry().clone();
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok(DhtOpLight::StoreEntry(s, h, e))
        }
        DhtOp::RegisterAgentActivity(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h).await?.into();
            Ok(DhtOpLight::RegisterAgentActivity(s, h))
        }
        DhtOp::RegisterReplacedBy(s, h, _) => {
            let e = h.entry_hash.clone();
            let old_entry = match h.update_basis {
                UpdateBasis::Entry => {
                    let entry = match cascade.dht_get_header_raw(&h.replaces_address).await? {
                        Some(header) => header.header().entry_data().map(|(hash, _)| hash.clone()),
                        None => todo!("try getting from the network"),
                    };
                    Some(entry.ok_or(DhtOpConvertError::MissingEntry)?)
                }
                _ => None,
            };
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            let op = RegisterReplacedByLight {
                signature: s,
                entry_update: h,
                new_entry: e,
                old_entry,
            };
            Ok(DhtOpLight::RegisterReplacedBy(op))
        }
        DhtOp::RegisterDeletedBy(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok(DhtOpLight::RegisterAgentActivity(s, h))
        }
        DhtOp::RegisterAddLink(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok(DhtOpLight::RegisterAgentActivity(s, h))
        }
        DhtOp::RegisterRemoveLink(s, h) => {
            let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
            Ok(DhtOpLight::RegisterAgentActivity(s, h))
        }
    }
}
