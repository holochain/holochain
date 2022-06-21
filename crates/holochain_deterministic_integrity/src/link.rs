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
        let out = zome_info()?.zome_types.links.all_dependencies();
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
    T: TryInto<ZomeId, Error = E>,
    T: Into<LinkType>,
    T: Copy,
    WasmError: From<E>,
{
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        let mut vec = self
            .into_iter()
            .map(|t| Ok((TryInto::<ZomeId>::try_into(t)?, Into::<LinkType>::into(t))))
            .collect::<Result<Vec<_>, _>>()?;
        vec.sort_unstable_by_key(|k| k.0);
        let vec = vec.into_iter().fold(
            Vec::new(),
            |mut out: Vec<(ZomeId, Vec<LinkType>)>, (zome_id, link_type)| {
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
    T: TryInto<ZomeId, Error = E>,
    T: Into<LinkType>,
    T: Copy,
    WasmError: From<E>,
{
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        self.to_vec().try_into_filter()
    }
}

impl<T, E, const N: usize> LinkTypeFilterExt for &[T; N]
where
    T: TryInto<ZomeId, Error = E>,
    T: Into<LinkType>,
    T: Copy,
    WasmError: From<E>,
{
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        self.to_vec().try_into_filter()
    }
}

impl<T, E> LinkTypeFilterExt for &[T]
where
    T: TryInto<ZomeId, Error = E>,
    T: Into<LinkType>,
    T: Copy,
    WasmError: From<E>,
{
    fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
        self.to_vec().try_into_filter()
    }
}
