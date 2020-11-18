use crate::prelude::*;

/// Deletes a CapGrant.
///
/// The input to delete_cap_grant evalutes to the HeaderHash of the CapGrant element to delete.
/// Deletes can reference both CapGrant creates and updates.
///
/// There are no branching CRUD trees for CapGrant entries because they are always local on the
/// current agent's source chain so there are no partitions or other ambiguity.
///
/// Deleting a CapGrant entry immediately revokes the referenced grant/secret.
///
/// Deletes cannot be reverted and secrets are unique across all grants and claims per chain.
///
/// To 'undo' a delete a new grant with a new secret will need to be issued.
///
/// @see create_cap_grant
pub fn delete_cap_grant(hash: HeaderHash) -> HdkResult<HeaderHash> {
    delete(hash)
}
