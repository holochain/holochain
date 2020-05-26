pub enum MetaGetStatus<T> {
    CanonicalHash(T),
    Deleted,
    NotFound,
}

#[cfg(test)]
mod tests {
    use crate::core::state::chain_meta::{ChainMetaBuf, ChainMetaBufT};
    use holo_hash::HeaderHash;
    use holochain_serialized_bytes::SerializedBytes;
    use holochain_state::{prelude::*, test_utils::test_cell_env};
    use holochain_types::{composite_hash::AnyDhtHash, header::EntryUpdate, Header};
    use std::convert::TryFrom;

    async fn test_update_header_once() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        {
            let reader = env.reader()?;
            let buf = ChainMetaBuf::primary(&reader, &env)?;
            let update: EntryUpdate = todo!("use HeaderBuilder, once possible");
            buf.add_update(update.clone())?;

            let canonical = if let AnyDhtHash::Header(original) = update.replaces_address {
                buf.get_canonical_header_hash(original)?
            } else {
                unreachable!()
            };

            let fetched = HeaderHash::with_data(
                SerializedBytes::try_from(Header::from(update))
                    .unwrap()
                    .bytes(),
            )
            .await;

            assert_eq!(canonical, fetched);
        }
    }
}
