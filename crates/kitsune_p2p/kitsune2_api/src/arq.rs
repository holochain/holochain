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
    /// Helper required for downcast.
    fn as_any(&self) -> Arc<dyn std::any::Any + 'static + Send + Sync>;

    /// Returns `true` if any parts of these two arqs overlap.
    fn overlap(&self, _oth: &DynArq) -> bool;

    /// Get the closest distance (in either direction) from the specified
    /// location to a covered part of this arq.
    /// - If this arq is empty, u32::MAX will be returned.
    /// - If this arq is full, 0 will be returned.
    fn dist(&self, _loc: u32) -> u32;
}

/// Trait-object [Arq].
pub type DynArq = Arc<dyn Arq>;

/// An empty arq.
#[derive(Debug)]
pub struct ArqEmpty(std::sync::Weak<Self>);

impl ArqEmpty {
    /// Construct an empty arq.
    pub fn create() -> DynArq {
        let out: DynArq = Arc::new_cyclic(|this| ArqEmpty(this.clone()));
        out
    }
}

impl Arq for ArqEmpty {
    fn as_any(&self) -> Arc<dyn std::any::Any + 'static + Send + Sync> {
        self.0.upgrade().expect("InvalidArc")
    }

    fn overlap(&self, _oth: &DynArq) -> bool {
        false
    }

    fn dist(&self, _loc: u32) -> u32 {
        u32::MAX
    }
}

/// A full arq.
#[derive(Debug)]
pub struct ArqFull(std::sync::Weak<Self>);

impl ArqFull {
    /// Construct a full arq.
    pub fn create() -> DynArq {
        let out: DynArq = Arc::new_cyclic(|this| ArqFull(this.clone()));
        out
    }
}

impl Arq for ArqFull {
    fn as_any(&self) -> Arc<dyn std::any::Any + 'static + Send + Sync> {
        self.0.upgrade().expect("InvalidArc")
    }

    fn overlap(&self, _oth: &DynArq) -> bool {
        true
    }

    fn dist(&self, _loc: u32) -> u32 {
        0
    }
}
