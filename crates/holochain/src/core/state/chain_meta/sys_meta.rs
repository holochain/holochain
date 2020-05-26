pub enum MetaGetStatus<T> {
    CanonicalHash(T),
    Deleted,
    NotFound,
}

#[cfg(test)]
mod tests {
    use crate::core::state::chain_meta::{ChainMetaBuf, ChainMetaBufT};
    use holo_hash::*;
    use holochain_serialized_bytes::SerializedBytes;
    use holochain_state::{prelude::*, test_utils::test_cell_env};
    use holochain_types::{
        composite_hash::{AnyDhtHash, EntryHash},
        header::EntryUpdate,
        Entry, EntryHashed, Header, HeaderHashed,
    };
    use std::convert::TryFrom;
    use unwrap_to::unwrap_to;

    /// Test that a header can be redirected a single hop
    async fn test_redirect_header_one_hop() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update: EntryUpdate = todo!("use HeaderBuilder, once possible");
            buf.add_update(update.clone())?;

            let original = unwrap_to!(update.replaces_address => AnyDhtHash::Header);
            let canonical = buf.get_canonical_header_hash(original.clone())?;

            let expected = HeaderHashed::with_data(Header::from(update)).await?;
            assert_eq!(canonical, *expected.as_hash());
        }
    }

    /// Test that a header can be redirected three hops
    async fn test_redirect_header_three_hops() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update1: EntryUpdate = todo!("use HeaderBuilder, once possible");
            let update2: EntryUpdate = todo!("use HeaderBuilder, once possible");
            let update3: EntryUpdate = todo!("use HeaderBuilder, once possible");
            buf.add_update(update1.clone())?;
            buf.add_update(update2.clone())?;
            buf.add_update(update3.clone())?;

            let original = unwrap_to!(update1.replaces_address => AnyDhtHash::Header);
            let canonical = buf.get_canonical_header_hash(original.clone())?;

            let expected = HeaderHashed::with_data(Header::from(update3)).await?;
            assert_eq!(canonical, *expected.as_hash());
        }
    }

    /// Test that an entry can be redirected a single hop
    async fn test_redirect_entry_one_hop() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update: EntryUpdate = todo!("use HeaderBuilder, once possible");
            buf.add_update(update.clone())?;

            let original: EntryHash =
                unwrap_to!(update.replaces_address => AnyDhtHash::EntryContent)
                    .clone()
                    .into();
            let canonical = buf.get_canonical_entry_hash(original)?;

            let expected = update.entry_hash;
            assert_eq!(canonical, expected);
        }
    }

    /// Test that an entry can be redirected three hops
    async fn test_redirect_entry_three_hops() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update1: EntryUpdate = todo!("use HeaderBuilder, once possible");
            let update2: EntryUpdate = todo!("use HeaderBuilder, once possible");
            let update3: EntryUpdate = todo!("use HeaderBuilder, once possible");
            buf.add_update(update1.clone())?;
            buf.add_update(update2.clone())?;
            buf.add_update(update3.clone())?;

            let original: EntryHash =
                unwrap_to!(update1.replaces_address => AnyDhtHash::EntryContent)
                    .clone()
                    .into();
            let canonical = buf.get_canonical_entry_hash(original)?;

            let expected = update3.entry_hash;
            assert_eq!(canonical, expected);
        }
    }

    /// Test that a header can be redirected a single hop
    async fn test_redirect_header_and_entry() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update_header: EntryUpdate = todo!("use HeaderBuilder, once possible");
            let update_entry: EntryUpdate = todo!("use HeaderBuilder, once possible");
            buf.add_update(update_header.clone())?;
            buf.add_update(update_entry.clone())?;

            let original_header_hash =
                unwrap_to!(update_header.replaces_address => AnyDhtHash::Header);
            let original_entry_hash =
                unwrap_to!(update_entry.replaces_address => AnyDhtHash::EntryContent)
                    .clone()
                    .into();
            let canonical_header_hash =
                buf.get_canonical_header_hash(original_header_hash.clone())?;
            let canonical_entry_hash = buf.get_canonical_entry_hash(original_entry_hash)?;

            let expected_header = HeaderHashed::with_data(Header::from(update_header)).await?;
            let expected_entry_hash = update_entry.entry_hash;
            assert_eq!(canonical_header_hash, *expected_header.as_hash());
            assert_eq!(canonical_entry_hash, expected_entry_hash);
        }
    }
}
