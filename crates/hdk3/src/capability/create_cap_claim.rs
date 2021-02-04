use crate::prelude::*;

/// Create a CapClaim on the local source chain.
///
/// Recipients of secrets for use in redemption of CapGrants use CapClaim entries to save it.
///
/// The `CapClaim` contains the secret, tag and issuing agent.
/// Only the secret should ever be sent back to the issuing agent, everything else is for local
/// administering, querying and filtering only.
///
/// There is no guarantee that any CapClaim is currently usable without trying it.
/// The author of the corresponding CapGrant can revoke it at any time, or simply be offline.
///
/// The `call_remote` will return a `ZomeCallResponse::Unauthorized` when the grantor considers the
/// secret invalid for the call. The caller is expected to handle this gracefully.
///
/// If the author of the CapGrant is online and has not revoked the grant, they will allow any
/// agent with a valid secret and pubkey to `call_remote` externs on their machine.
///
/// @see CapClaim
/// @see cap grant functions
pub fn create_cap_claim(cap_claim_entry: CapClaimEntry) -> ExternResult<HeaderHash> {
    create(EntryWithDefId::new(
        EntryDefId::CapClaim,
        Entry::CapClaim(cap_claim_entry),
    ))
}
