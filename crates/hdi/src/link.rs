use std::collections::HashMap;

use holochain_integrity_types::LinkTypeFilter;
use holochain_wasmer_guest::WasmError;

use crate::prelude::*;

#[cfg(doc)]
pub mod examples;

pub trait LinkTypeFilterExt {
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError>;
}

impl LinkTypeFilterExt for core::ops::RangeFull {
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        let out = zome_info()?.zome_types.links.dependencies().collect();
        Ok(LinkTypeFilter::Dependencies(out))
    }
}

impl LinkTypeFilterExt for LinkTypeFilter {
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        Ok(self)
    }
}

impl<T, E> LinkTypeFilterExt for Vec<T>
where
    T: TryInto<ScopedLinkType, Error = E>,
    WasmError: From<E>,
{
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        // Collect into a 2d vector of where `LinkType`s are collected
        // into their common `ZomeId`s.
        let vec = self
            .into_iter()
            .try_fold(HashMap::new(), |mut map: HashMap<_, Vec<_>>, t| {
                let scoped = TryInto::<ScopedLinkType>::try_into(t)?;
                map.entry(scoped.zome_id)
                    .or_default()
                    .push(scoped.zome_type);
                Ok(map)
            })?
            .into_iter()
            .collect::<Vec<(_, Vec<_>)>>();

        Ok(LinkTypeFilter::Types(vec))
    }
}

impl<T, E, const N: usize> LinkTypeFilterExt for [T; N]
where
    T: TryInto<ScopedLinkType, Error = E>,
    WasmError: From<E>,
{
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        self.into_iter()
            .map(TryInto::<ScopedLinkType>::try_into)
            .collect::<Result<Vec<_>, _>>()?
            .try_into_filter()
    }
}

impl<T, E, const N: usize> LinkTypeFilterExt for &[T; N]
where
    for<'a> &'a T: TryInto<ScopedLinkType, Error = E>,
    WasmError: From<E>,
{
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        self.iter()
            .map(TryInto::<ScopedLinkType>::try_into)
            .collect::<Result<Vec<_>, _>>()?
            .try_into_filter()
    }
}

impl<T, E> LinkTypeFilterExt for &[T]
where
    for<'a> &'a T: TryInto<ScopedLinkType, Error = E>,
    WasmError: From<E>,
{
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        self.iter()
            .map(TryInto::<ScopedLinkType>::try_into)
            .collect::<Result<Vec<_>, _>>()?
            .try_into_filter()
    }
}
