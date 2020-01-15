use crate::{cell::Cell, shims::*};
use holochain_persistence_api::cas::content::Address;
use skunkworx_core_types::error::SkunkResult;

/// Representation of a Cell's source chain.
/// TODO: work out the details of what's needed for as_at
/// to make sure the right balance is struck between
/// creating as_at snapshots and having access to the actual current source chain
pub struct SourceChain {
    cell: Cell,
    as_at: Option<Address>,
}

impl SourceChain {
    /// Fails if a source chain has not yet been created for this CellId.
    pub fn from_cell(cell: Cell) -> SkunkResult<Self> {
        // TODO: fail if non existant
        Ok(Self { cell, as_at: None })
    }

    /// Return new SourceChain with head address `as_at` specified address
    /// This is a potentially truncated snapshot of the actual source chain
    /// Fails if `as_at` is not in the CAS
    pub fn as_at(mut self, as_at: Address) -> SkunkResult<Self> {
        // TODO: check if as_at is in CAS
        self.as_at = Some(as_at);
        Ok(self)
    }

    pub fn as_at_head(self) -> SkunkResult<Self> {
        let actual_head = self.persisted_head_address();
        self.as_at(actual_head)
    }

    pub fn get_dna(&self) -> SkunkResult<Dna> {
        Ok(Dna)
    }

    // pub fn _head_address(&self) -> Address {
    //     self.as_at
    //         .clone()
    //         .unwrap_or_else(|| self.persisted_head_address())
    // }

    fn persisted_head_address(&self) -> Address {
        // TODO: read persisted head address from CAS
        unimplemented!()
    }

    /// Use the SCHH to attempt to write a bundle of changes
    pub fn try_commit(&self, cursor: CascadingCursor) -> SkunkResult<()> {
        unimplemented!()
    }
}
