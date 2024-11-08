//! Arq-related types.

use crate::*;

/// A sparse concept of coverage.
/// This type could represent no coverage at all, or complete total coverage.
/// It could also represent any granularity or count of disparate sparse
/// mixed regeions of coverage and no coverage.
//
// In legacy kitsune, this type could have been backed by any of:
// - ArqRange
// - DhtArq
// - DhtArqSet
pub trait Arq: 'static + Send + Sync + std::fmt::Debug {
    /// Returns the full set of inclusive bounds defined by this Arq.
    /// - If this Arq is empty, it returns `[]`.
    /// - If this Arq is full, it returns `[(0, u32::MAX)]`.
    fn inclusive_bounds(&self) -> &[(u32, u32)];

    /// Returns `true` if any parts of these two arqs overlap.
    fn overlap(&self, _oth: &DynArq) -> bool {
        // TODO - actually implement this
        false
    }

    /// Get the closest distance (in either direction) from the specified
    /// location to a covered part of this arq.
    /// - If this arq is empty, u32::MAX will be returned.
    /// - If this arq is full, 0 will be returned.
    fn dist(&self, _loc: u32) -> u32 {
        // TODO - actually implement this
        u32::MAX
    }
}

/// Trait-object [Arq].
pub type DynArq = Arc<dyn Arq>;

/// Arq constructed directly from inclusive bounds.
#[derive(Debug)]
pub struct InclusiveBoundArq(pub Box<[(u32, u32)]>);

impl InclusiveBoundArq {
    /// Construct this Arq from a Vec.
    pub fn from_vec(b: Vec<(u32, u32)>) -> DynArq {
        Arc::new(Self(b.into_boxed_slice()))
    }
}

impl Arq for InclusiveBoundArq {
    fn inclusive_bounds(&self) -> &[(u32, u32)] {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty_does_not_overlap() {
        assert!(!InclusiveBoundArq::from_vec(vec![])
            .overlap(&InclusiveBoundArq::from_vec(vec![(0, u32::MAX)])));
    }
}
