//! Helpers for constructing and using zome types correctly.
use std::collections::HashMap;

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

/// The number of types of a given type per zome.
pub type NumZomeTypes = u8;
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
                zome_types.entries.insert(zome_id, num_entry_types.0);
                zome_types.links.insert(zome_id, num_link_types.0);
                Ok(zome_types)
            },
        )?;
        Ok(r)
    }

    /// Create a new zome types map within the scope of the given integrity zomes.
    pub fn in_scope_subset(&self, zomes: &[ZomeId]) -> ScopedZomeTypesSet {
        let entries = zomes
            .iter()
            .filter_map(|zome_id| self.entries.get_key_value(zome_id).map(|(z, l)| (*z, *l)));
        let entries = new_scope(entries);
        let links = zomes
            .iter()
            .filter_map(|zome_id| self.links.get_key_value(zome_id).map(|(z, l)| (*z, *l)));
        let links = new_scope(links);
        ScopedZomeTypesSet { entries, links }
    }
}

fn new_scope<T>(iter: impl Iterator<Item = (ZomeId, NumZomeTypes)>) -> ScopedZomeTypes<T>
where
    T: From<u8>,
{
    let iter = iter
        .map(|(zome_id, len)| (zome_id, (0..len).map(Into::into).collect()))
        .collect();
    ScopedZomeTypes(iter)
}
