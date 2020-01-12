use crate::{cell::CellId, shims::CascadingCursor};
use futures::never::Never;
use holochain_persistence_api::cas::content::Address;

/// Representation of a Cell's source chain.
/// TODO: work out the details of what's needed for as_at
/// to make sure the right balance is struck between
/// creating as_at snapshots and having access to the actual current source chain
pub struct SourceChain {
    cell_id: CellId,
    as_at: Option<Address>,
}

impl SourceChain {
    /// Fails if a source chain has not yet been created for this CellId.
    pub fn from_cell_id(cell_id: CellId) -> Result<Self, Never> {
        // TODO: fail if non existant
        Ok(Self {
            cell_id,
            as_at: None,
        })
    }

    /// Return new SourceChain with head address `as_at` specified address
    /// This is a potentially truncated snapshot of the actual source chain
    /// Fails if `as_at` is not in the CAS
    pub fn as_at(mut self, as_at: Address) -> Result<Self, Never> {
        // TODO: check if as_at is in CAS
        self.as_at = Some(as_at);
        Ok(self)
    }

    pub fn as_at_head(self) -> Result<Self, Never> {
        let actual_head = self.persisted_head_address();
        self.as_at(actual_head)
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
    pub fn try_commit(&self, cursor: CascadingCursor) -> Result<(), Never> {
        unimplemented!()
    }
}
