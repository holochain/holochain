//! Helpers for constructing and using zome types correctly.
use std::ops::Range;

pub use error::*;
use holochain_zome_types::EntryDefIndex;
use holochain_zome_types::GlobalZomeTypeId;
use holochain_zome_types::LinkType;
use holochain_zome_types::ScopedZomeTypes;
use holochain_zome_types::ScopedZomeTypesSet;
use holochain_zome_types::ZomeId;

#[allow(missing_docs)]
mod error;

/// Zome types at the global scope for a DNA.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GlobalZomeTypes(ScopedZomeTypesSet);

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
        let r = ordered_iterator.into_iter().try_fold(
            ScopedZomeTypesSet::default(),
            |mut zome_types, (num_entry_types, num_link_types)| {
                let start = zome_types
                    .entries
                    .0
                    .last()
                    .map(|r| r.end)
                    .unwrap_or(GlobalZomeTypeId(0));
                let end = start
                    .0
                    .checked_add(num_entry_types.0)
                    .ok_or(ZomeTypesError::EntryTypeIndexOverflow)?
                    .into();
                zome_types.entries.0.push(start..end);
                let start = zome_types
                    .links
                    .0
                    .last()
                    .map(|r| r.end)
                    .unwrap_or(GlobalZomeTypeId(0));
                let end = start
                    .0
                    .checked_add(num_link_types.0)
                    .ok_or(ZomeTypesError::EntryTypeIndexOverflow)?
                    .into();
                zome_types.links.0.push(start..end);
                Ok(zome_types)
            },
        )?;
        Ok(GlobalZomeTypes(r))
    }

    /// TODO
    pub fn re_scope(&self, zomes: &[ZomeId]) -> ZomeTypesResult<ScopedZomeTypesSet> {
        let Self(ScopedZomeTypesSet { entries, links }) = self;
        let entries = zomes
            .iter()
            .map(|zome_id| {
                entries
                    .0
                    .get(zome_id.0 as usize)
                    .cloned()
                    .ok_or_else(|| ZomeTypesError::MissingZomeType(*zome_id))
            })
            .collect::<ZomeTypesResult<Vec<_>>>()?;
        let links = zomes
            .iter()
            .map(|zome_id| {
                links
                    .0
                    .get(zome_id.0 as usize)
                    .cloned()
                    .ok_or_else(|| ZomeTypesError::MissingZomeType(*zome_id))
            })
            .collect::<ZomeTypesResult<Vec<_>>>()?;
        Ok(ScopedZomeTypesSet {
            entries: ScopedZomeTypes(entries),
            links: ScopedZomeTypes(links),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_is_deterministic() {
        let zome_types = vec![
            (EntryDefIndex(3), LinkType(2)),
            (EntryDefIndex(0), LinkType(0)),
            (EntryDefIndex(5), LinkType(1)),
            (EntryDefIndex(12), LinkType(0)),
        ];

        assert_eq!(
            GlobalZomeTypes::from_ordered_iterator(zome_types.clone()).unwrap(),
            GlobalZomeTypes::from_ordered_iterator(zome_types.clone()).unwrap(),
        );

        let mut expect = GlobalZomeTypes::default();

        expect
            .0
            .entries
            .0
            .push(GlobalZomeTypeId(0)..GlobalZomeTypeId(3));
        expect
            .0
            .entries
            .0
            .push(GlobalZomeTypeId(3)..GlobalZomeTypeId(3));
        expect
            .0
            .entries
            .0
            .push(GlobalZomeTypeId(3)..GlobalZomeTypeId(8));
        expect
            .0
            .entries
            .0
            .push(GlobalZomeTypeId(8)..GlobalZomeTypeId(20));

        expect
            .0
            .links
            .0
            .push(GlobalZomeTypeId(0)..GlobalZomeTypeId(2));
        expect
            .0
            .links
            .0
            .push(GlobalZomeTypeId(2)..GlobalZomeTypeId(2));
        expect
            .0
            .links
            .0
            .push(GlobalZomeTypeId(2)..GlobalZomeTypeId(3));
        expect
            .0
            .links
            .0
            .push(GlobalZomeTypeId(3)..GlobalZomeTypeId(3));

        assert_eq!(
            GlobalZomeTypes::from_ordered_iterator(zome_types).unwrap(),
            expect
        )
    }
}
