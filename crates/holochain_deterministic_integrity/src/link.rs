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
        let mut vec = self
            .into_iter()
            .map(TryInto::<ScopedLinkType>::try_into)
            .collect::<Result<Vec<_>, _>>()?;
        vec.sort_unstable();
        let vec = vec.into_iter().fold(
            Vec::new(),
            |mut out: Vec<(ZomeId, Vec<LinkType>)>,
             ScopedZomeType {
                 zome_id,
                 zome_type: link_type,
             }| {
                match out.last_mut() {
                    Some(l) if l.0 == zome_id => l.1.push(link_type),
                    _ => out.push((zome_id, vec![link_type])),
                }
                out
            },
        );
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
