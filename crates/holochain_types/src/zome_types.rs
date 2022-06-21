//! Helpers for constructing and using zome types correctly.
use std::collections::HashMap;
use std::num::NonZeroU8;

pub use error::*;
use holochain_zome_types::EntryDefIndex;
use holochain_zome_types::LinkType;
use holochain_zome_types::ScopedZomeTypes;
use holochain_zome_types::ScopedZomeTypesSet;
use holochain_zome_types::ZomeId;

#[allow(missing_docs)]
mod error;
#[cfg(test)]
mod test;

/// TODO
pub type NumZomeTypes = NonZeroU8;
/// Zome types at the global scope for a DNA.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GlobalZomeTypes {
    entries: HashMap<ZomeId, NumZomeTypes>,
    links: HashMap<ZomeId, NumZomeTypes>,
}

impl GlobalZomeTypes {
    /// Create a new zome types map from the order of
    /// the iterators. The iterator must be the same order
    /// as the integrity zomes.
    ///
    /// This iterator should contain the number of [`EntryDefIndex`] and [`LinkType`]
    /// for each integrity zome. If the zome does not have any entries or links,
    /// then it should still have a zero value set.
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
        let r = ordered_iterator.into_iter().enumerate().try_fold(
            Self::default(),
            |mut zome_types, (zome_id, (num_entry_types, num_link_types))| {
                let zome_id: ZomeId = u8::try_from(zome_id)
                    .map_err(|_| ZomeTypesError::ZomeIndexOverflow)?
                    .into();
                if let Some(num_entry_types) = NonZeroU8::new(num_entry_types.0) {
                    zome_types.entries.insert(zome_id, num_entry_types);
                }
                if let Some(num_link_types) = NonZeroU8::new(num_link_types.0) {
                    zome_types.links.insert(zome_id, num_link_types);
                }
                Ok(zome_types)
            },
        )?;
        Ok(r)
    }

    /// Create a new zome types map within the scope of the given integrity zomes.
    pub fn re_scope(&self, zomes: &[ZomeId]) -> ZomeTypesResult<ScopedZomeTypesSet> {
        let entries = zomes
            .iter()
            .filter_map(|zome_id| self.entries.get_key_value(zome_id).map(|(z, l)| (*z, *l)));
        let entries = new_scope(entries).ok_or(ZomeTypesError::EntryTypeIndexOverflow)?;
        let links = zomes
            .iter()
            .filter_map(|zome_id| self.links.get_key_value(zome_id).map(|(z, l)| (*z, *l)));
        let links = new_scope(links).ok_or(ZomeTypesError::LinkTypeIndexOverflow)?;
        Ok(ScopedZomeTypesSet { entries, links })
    }
}

fn new_scope(iter: impl Iterator<Item = (ZomeId, NumZomeTypes)>) -> Option<ScopedZomeTypes> {
    let mut total: u8 = 0;
    let iter = iter
        .map(|(zome_id, len)| {
            let len = len.get();
            total = total.checked_add(len)?;
            // Safe because len is never zero
            let last_index = total - 1;
            Some((last_index.into(), zome_id))
        })
        .collect::<Option<_>>()?;
    Some(ScopedZomeTypes(iter))
}
