pub enum MetaGetStatus<T> {
    CanonicalHash(T),
    Deleted,
    NotFound,
}

#[cfg(test)]
mod tests {
    use crate::core::state::chain_meta::{ChainMetaBuf, ChainMetaBufT};
    use fixt::prelude::*;
    use holo_hash::*;
    use holochain_serialized_bytes::SerializedBytes;
    use holochain_state::{prelude::*, test_utils::test_cell_env};
    use holochain_types::{
        composite_hash::{AnyDhtHash, EntryHash},
        fixt::HeaderBuilderCommonFixturator,
        header::{builder, EntryUpdate, HeaderBuilder},
        Entry, EntryHashed, Header, HeaderHashed,
    };
    use unwrap_to::unwrap_to;

    /// Test that a header can be redirected a single hop
    async fn test_redirect_header_one_hop() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let fixt = HeaderBuilderCommonFixturator::new(Predictable);
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());
            let expected = buf.add_update(update.clone())?;

            let original = unwrap_to!(update.replaces_address => AnyDhtHash::Header);
            let canonical = buf.get_canonical_header_hash(original.clone())?;

            assert_eq!(canonical, expected);
        }
    }

    /// Test that a header can be redirected three hops
    async fn test_redirect_header_three_hops() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let fixt = HeaderBuilderCommonFixturator::new(Predictable);
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update1 = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());
            let update2 = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());
            let update3 = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());
            let _ = buf.add_update(update1.clone())?;
            let _ = buf.add_update(update2)?;
            let expected = buf.add_update(update3.clone())?;

            let original = unwrap_to!(update1.replaces_address => AnyDhtHash::Header);
            let canonical = buf.get_canonical_header_hash(original.clone())?;

            assert_eq!(canonical, expected);
        }
    }

    /// Test that an entry can be redirected a single hop
    async fn test_redirect_entry_one_hop() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let fixt = HeaderBuilderCommonFixturator::new(Predictable);
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());
            let _ = buf.add_update(update.clone())?;

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
        let fixt = HeaderBuilderCommonFixturator::new(Predictable);
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update1 = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());
            let update2 = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());
            let update3 = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());
            let _ = buf.add_update(update1.clone())?;
            let _ = buf.add_update(update2.clone())?;
            let _ = buf.add_update(update3.clone())?;

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
        let fixt = HeaderBuilderCommonFixturator::new(Predictable);
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let update_header = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());
            let update_entry = builder::EntryUpdate {
                replaces_address: todo!(),
                entry_type: todo!(),
                entry_hash: todo!(),
            }
            .build(fixt.next().unwrap());

            let expected_header_hash = buf.add_update(update_header.clone())?;
            let _ = buf.add_update(update_entry.clone())?;
            let expected_entry_hash = update_entry.entry_hash;

            let original_header_hash =
                unwrap_to!(update_header.replaces_address => AnyDhtHash::Header);
            let original_entry_hash =
                unwrap_to!(update_entry.replaces_address => AnyDhtHash::EntryContent)
                    .clone()
                    .into();
            let canonical_header_hash =
                buf.get_canonical_header_hash(original_header_hash.clone())?;
            let canonical_entry_hash = buf.get_canonical_entry_hash(original_entry_hash)?;

            assert_eq!(canonical_header_hash, expected_header_hash);
            assert_eq!(canonical_entry_hash, expected_entry_hash);
        }
    }
}
