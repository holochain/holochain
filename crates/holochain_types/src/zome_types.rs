//! Helpers for constructing and using zome types correctly.
use std::ops::Range;

pub use error::*;
use holochain_zome_types::AppEntryDefName;
use holochain_zome_types::EntryDefIndex;
use holochain_zome_types::GlobalZomeTypeId;
use holochain_zome_types::LinkType;
use holochain_zome_types::LinkTypeName;
use holochain_zome_types::ScopedZomeType;
use holochain_zome_types::ScopedZomeTypes;
use holochain_zome_types::ZomeId;

#[allow(missing_docs)]
mod error;

/// Zome types at the global scope for a DNA.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GlobalZomeTypes(ScopedZomeTypes);

impl GlobalZomeTypes {
    /// Create a new zome types map from the order of
    /// the iterators. The iterator must be the same order
    /// as the integrity zomes.
    ///
    /// # Correct Usage
    /// You must use an iterator with a deterministic order.
    ///
    /// For example [`HashMap`](std::collections::HashMap) does not produce
    /// deterministic iterators so should not be used as the source.
    pub fn from_ordered_iterator<I>(ordered_iterator: I) -> ZomeTypesResult<GlobalZomeTypes>
    where
        I: IntoIterator<Item = (EntryDefIndex, LinkType)>,
    {
        let r = ordered_iterator
            .into_iter()
            .try_fold(
                ScopedZomeTypes::default(),
                |mut zome_types, (num_entry_types, num_link_types)| {
                    let start = zome_types
                        .entries
                        .0
                        .last()
                        .map(|r| r.end)
                        .unwrap_or(0.into());
                    let end = start.0.checked_add(num_entry_types.0)?.into();
                    zome_types.entries.0.push(start..end);
                    let start = zome_types.links.0.last().map(|r| r.end).unwrap_or(0.into());
                    let end = start.0.checked_add(num_link_types.0)?.into();
                    zome_types.links.0.push(start..end);
                    Some(zome_types)
                },
            )
            // FIXME: Make error
            .unwrap();
        Ok(GlobalZomeTypes(r))
    }

    /// TODO
    pub fn re_scope(&self, zomes: &[ZomeId]) -> ZomeTypesResult<ScopedZomeTypes> {
        let Self(ScopedZomeTypes { entries, links }) = self;
        let entries = zomes
            .iter()
            .map(|zome_id| entries.0.get(zome_id.0 as usize).cloned())
            .collect::<Option<Vec<_>>>()
            // FIXME: Make error
            .unwrap();
        let links = zomes
            .iter()
            .map(|zome_id| links.0.get(zome_id.0 as usize).cloned())
            .collect::<Option<Vec<_>>>()
            // FIXME: Make error
            .unwrap();
        Ok(ScopedZomeTypes {
            entries: ScopedZomeType(entries),
            links: ScopedZomeType(links),
        })
    }

    /// TODO
    pub fn find_zome_id_from_entry(&self, entry_index: &EntryDefIndex) -> Option<ZomeId> {
        find_zome_id(self.0.entries.0.iter(), &(*entry_index).into())
    }

    /// TODO
    pub fn find_zome_id_from_link(&self, link_index: &LinkType) -> Option<ZomeId> {
        find_zome_id(self.0.links.0.iter(), &(*link_index).into())
    }
}

fn find_zome_id<'iter>(
    iter: impl Iterator<Item = &'iter Range<GlobalZomeTypeId>>,
    index: &GlobalZomeTypeId,
) -> Option<ZomeId> {
    iter.enumerate().find_map(|(i, range)| {
        range
            .contains(index)
            .then(|| i)
            .and_then(|i| Some(ZomeId(i.try_into().ok()?)))
    })
}

// /// A helper trait to correctly construct a
// /// [`ZomeTypesMap`]
// pub trait ZomeTypesMapConstructor {
//     /// Create a new zome types map from the order of
//     /// the iterators. The iterator must be the same order
//     /// as the integrity zomes.
//     ///
//     /// # Correct Usage
//     /// You must use an iterator with a deterministic order.
//     ///
//     /// For example [`HashMap`](std::collections::HashMap) does not produce
//     /// deterministic iterators so should not be used as the source.
//     fn from_ordered_iterator<I, E, L, EN, LN>(ordered_iterator: I) -> ZomeTypesResult<ZomeTypesMap>
//     where
//         I: IntoIterator<Item = (E, L)>,
//         E: IntoIterator<Item = EN>,
//         L: IntoIterator<Item = LN>,
//         AppEntryDefName: From<EN>,
//         LinkTypeName: From<LN>,
//     {
//         // Start with both indices at zero.
//         let mut entry_type_index = 0u8;
//         let mut link_type_index = 0u8;

//         let map = ordered_iterator
//             .into_iter()
//             .enumerate()
//             .map(|(zome_index, (entry_iter, link_iter))| {
//                 let zome_id = ZomeId(
//                     zome_index
//                         .try_into()
//                         .map_err(|_| ZomeTypesError::ZomeIndexOverflow)?,
//                 );
//                 let entry = entry_iter
//                     .into_iter()
//                     .map(|name| {
//                         map_name_to_index(name, &mut entry_type_index)
//                             .ok_or(ZomeTypesError::EntryTypeIndexOverflow)
//                     })
//                     .collect::<Result<_, _>>()?;
//                 let link = link_iter
//                     .into_iter()
//                     .map(|name| {
//                         map_name_to_index(name, &mut link_type_index)
//                             .ok_or(ZomeTypesError::LinkTypeIndexOverflow)
//                     })
//                     .collect::<Result<_, _>>()?;
//                 Ok((zome_id, ZomeTypes { entry, link }))
//             })
//             .collect::<ZomeTypesResult<_>>()?;
//         Ok(ZomeTypesMap(map))
//     }
// }

// fn map_name_to_index<T, N, I>(name: T, index: &mut u8) -> Option<(N, I)>
// where
//     N: From<T>,
//     I: From<u8>,
// {
//     let i = *index;
//     *index = index.checked_add(1)?;
//     Some((name.into(), i.into()))
// }

// impl ZomeTypesMapConstructor for ZomeTypesMap {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn construction_is_deterministic() {
        let zome_types = vec![
            (vec!["a", "b", "c"], vec!["a", "b"]),
            (vec![], vec![]),
            (vec!["a", "b", "c"], vec!["a", "b"]),
            (vec!["d", "b", "c"], vec!["t", "b", "f"]),
        ];

        assert_eq!(
            ZomeTypesMap::from_ordered_iterator(zome_types.clone()).unwrap(),
            ZomeTypesMap::from_ordered_iterator(zome_types.clone()).unwrap(),
        );

        let mut expect = HashMap::new();

        let mut e = HashMap::new();
        let mut l = HashMap::new();

        e.insert("a".into(), 0.into());
        e.insert("b".into(), 1.into());
        e.insert("c".into(), 2.into());

        l.insert("a".into(), 0.into());
        l.insert("b".into(), 1.into());

        expect.insert(ZomeId(0), ZomeTypes { entry: e, link: l });

        expect.insert(ZomeId(1), Default::default());

        let mut e = HashMap::new();
        let mut l = HashMap::new();

        e.insert("a".into(), 3.into());
        e.insert("b".into(), 4.into());
        e.insert("c".into(), 5.into());

        l.insert("a".into(), 2.into());
        l.insert("b".into(), 3.into());

        expect.insert(ZomeId(2), ZomeTypes { entry: e, link: l });

        let mut e = HashMap::new();
        let mut l = HashMap::new();

        e.insert("d".into(), 6.into());
        e.insert("b".into(), 7.into());
        e.insert("c".into(), 8.into());

        l.insert("t".into(), 4.into());
        l.insert("b".into(), 5.into());
        l.insert("f".into(), 6.into());

        expect.insert(ZomeId(3), ZomeTypes { entry: e, link: l });

        assert_eq!(
            ZomeTypesMap::from_ordered_iterator(zome_types).unwrap(),
            ZomeTypesMap(expect)
        )
    }
}
