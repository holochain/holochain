use core::array::IntoIter;
use core::ops::RangeBounds;

use crate::prelude::*;

/// A helper trait for creating [`LinkTypeRanges`] that match the local zome's
/// type scope.
///
/// This is implemented by the [`hdk_link_types`] proc_macro.
pub trait LinkTypesHelper<const LEN: usize>: EnumLen
where
    Self: Into<LocalZomeTypeId>,
    Self: std::fmt::Debug + Clone + Copy + Sized + PartialEq + PartialOrd + 'static,
{
    /// Create a [`LinkTypeRanges`] from a range of this traits implementor.
    fn range(
        range: impl RangeBounds<Self> + 'static + std::fmt::Debug,
    ) -> Box<dyn FnOnce() -> Result<LinkTypeRanges, WasmError>> {
        let zome_types = zome_info().map(|t| t.zome_types);
        let f = move || {
            let zome_types = zome_types?;

            Ok(Self::find_variant(|_| true, &range, &zome_types)?.into())
        };
        Box::new(f)
    }

    #[doc(hidden)]
    fn find_variant(
        mut filter: impl FnMut(&Self) -> bool,
        range: &(impl std::ops::RangeBounds<Self> + 'static + std::fmt::Debug),
        zome_types: &ScopedZomeTypesSet,
    ) -> Result<LinkTypeRange, WasmError> {
        let start = Self::iter().filter(&mut filter).find(|t| range.contains(t));
        match start {
            Some(start) => {
                let end = Self::iter()
                    .filter(&mut filter)
                    .rev()
                    .find(|t| range.contains(t))
                    .unwrap_or(start);
                let start = zome_types.links.to_global_scope(start).ok_or_else(|| {
                    WasmError::Guest(format!(
                        "Unable to map start of range local zome type {:?} to global zome type scope",
                        start
                    ))
                })?;
                let end = zome_types.links.to_global_scope(end).ok_or_else(|| {
                    WasmError::Guest(format!(
                        "Unable to map end of range local zome type {:?} to global zome type scope",
                        end
                    ))
                })?;
                Ok(LinkTypeRange::Inclusive(
                    LinkType::from(start)..=LinkType::from(end),
                ))
            }
            None => Ok(LinkTypeRange::Empty),
        }
    }

    /// Iterate over all variants of this enum.
    fn iter() -> IntoIter<Self, LEN>;
}
