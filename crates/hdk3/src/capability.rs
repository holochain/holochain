use crate::prelude::*;

/// Create capability claims on the local source chain.
///
/// Wraps the `create` HDK3 function with system type parameters set.
/// This guards against sending application entry data or setting the wrong entry type.
///
/// Capability claims are grant _secrets_ that have been received from someone else.
/// The grant entry is never sent, only the associated secret.
/// The claim should be created on the local source chain so that it can be retrieved for later use.
///
/// Grantees of CapGrant secrets use CapClaim entries to save the secret.
///
/// The `CapClaim` contains the secret, tag and issuing agent.
/// Only the secret should ever be sent back to the issuing agent, everything else is only for local
/// administering, querying and filtering.
///
/// There is no guarantee that any CapClaim is currently usable without trying it.
/// The author of the corresponding CapGrant can revoke it at any time or be unreachable on the network.
///
/// Any [`call_remote`] will return a `ZomeCallResponse::Unauthorized` when the grantor considers the
/// secret invalid for the call. The caller is expected to handle this gracefully.
///
/// If the author of the CapGrant is reachable on the network and has not revoked the grant they will allow any
/// agent with a valid secret and pubkey to `call_remote` externs on the grant author's machine.
///
/// Functions executed on the grant author's machine that change the grantor's source chain will be signed by
/// the grantor.
/// Delegating responsibility to grantee claimants is a serious responsibility!
///
/// @see CapClaim
/// @see other cap grant functions
pub fn create_cap_claim(cap_claim_entry: CapClaimEntry) -> ExternResult<HeaderHash> {
    create(EntryWithDefId::new(
        EntryDefId::CapClaim,
        Entry::CapClaim(cap_claim_entry),
    ))
}

/// Create a capability grant.
///
/// Wraps the `create` HDK3 function with system type parameters set.
/// This guards against sending application entry data or setting the wrong entry type.
///
/// Capability grants are explicit entries in the local source chain that grant access to functions running in the current conductor.
/// The grant must be sent (e.g. with a remote call) to the grantees so they can commit a claim and then call back with it in the future.
///
/// When an agent wants to expose externs to be called remotely by other agents they need to select
/// a security model and probably generate a secret.
///
/// The input needs to evalute to a `ZomeCallCapGrant` struct which defines the tag, access and
/// granted zome/function pairs. The access is a `CapAccess` enum with variants `Unrestricted`,
/// `Transferable`, and `Assigned`.
///
/// The tag is an arbitrary `String` that developers or users can use to categorise and administer
/// grants committed to the chain. The tag should also match the `CapClaim` tags committed on the
/// recipient chain when a `CapGrant` is committed and shared. The tags are not checked or compared
/// in any security sensitive contexts.
///
/// Provided the grant author agent is is reachable on the network:
///
/// - The author always has access to all their own extern calls, bypassing `CapAccess`
/// - Unrestricted access means any external agent can call the extern
/// - Transferable access means any external agent with a valid secret can call the extern
/// - Assigned access means only explicitly approved agents with a valid secret can call the extern
///
/// All logic runs on the author agent's machine against their own source chain:
///
/// - New entries are committed to the author's chain with the author's signature
/// - Signals are emmitted to the author's system and GUI
/// - The author must be online from the perspective of the caller
/// - The author can chain `call_remote` back to the caller or any other agent
///
/// The happ developer needs to plan carefully to ensure auditability and accountability is
/// maintained for all writes and network calls if this is important to the integrity of the happ.
///
/// Multiple CapGrant entries can be relevant to a single attempted zome call invocation.
/// The most specific and strict CapGrant that validates will be used. For example, if a user
/// provided a valid transferable secret to a function that is currently unrestricted, the zome
/// call will be executed with the stricter transferable access.
///
/// @todo this is more relevant when partial application exists in the future
/// @todo predictably disambiguate multiple CapGrants of the same specificity
///
/// CapGrant entries can be updated and deleted in the same way as standard app entries.
/// The CRUD model for CapGrants is much simpler than app entries:
///
/// - versions are always local to a single source chain so partitions can never happen
/// - updates function like delete+create so that old grants are immediately revoked by a new grant
/// - deletes immediately revoke the referenced grant
/// - version histories are linear so there can never be a branching history of updates and deletes
///
/// @todo ensure linear history in sys validation
///
/// Secrets must be unique across all grants and claims in a source chain and should be generated
/// using the `generate_cap_secret` function that sources the correct number of cryptographically
/// strong random bytes from the host.
///
/// @todo ensure uniqueness of secrets in sys validation
///
/// If _any_ CapGrant is valid for a zome call invocation it will execute. Given that secrets must
/// be unique across all grants and claims this is easy to ensure for assigned and transferable
/// access. Special care is required for Unrestricted grants as several may apply to a single
/// extern at one time, or may apply in addition to a stricter grant. In this case, revoking a
/// stricter grant, or failing to revoke all Unrestricted grants will leave the function open.
///
/// @todo administration functions to query active grants
///
/// There is an apparent "chicken or the egg" situation where CapGrants are required for remote
/// agents to call externs, so how does an agent request a grant in the first place?
/// The simplest pattern is for agents to create an extern dedicated to assess incoming grant
/// requests and to apply `Unrestricted` access to it during the zome's `init` callback.
/// If Alice wants access to Bob's `foo` function she first grants Bob `Assigned` access to her own
/// `accept_foo_grant` extern and sends her grant's secret to Bob's `issue_foo_grant` function. Bob
/// receives Alice's request and, if he is willing to grant Alice access, he commits Alice's secret
/// as a `CapClaim` to his chain. Bob then generates a new secret and commits it in a `CapGrant`
/// for `foo`, most likely explicitly `Assigned` to Alice, and sends his secret and Alice's secret
/// to Alice's `accept_foo_grant` extern. Alice checks her grant, which matches Bob's public key
/// and the secret Bob received from her, then she commits a new CapClaim including the secret that
/// Bob generated. Now Alice can call `foo` on Bob's machine any time he is online, and because all
/// the secrets are `Assigned` Bob can track and update exactly who has access to his externs.
///
/// @see ZomeCallCapGrant
/// @see CapAccess
/// @see create_cap_claim
/// @see generate_cap_secret
pub fn create_cap_grant(cap_grant_entry: CapGrantEntry) -> ExternResult<HeaderHash> {
    create(EntryWithDefId::new(
        EntryDefId::CapGrant,
        Entry::CapGrant(cap_grant_entry),
    ))
}

/// Delete a capability grant.
///
/// Wraps the `delete` HDK3 function with system type parameters set.
/// This guards against deleting application entries or setting the wrong entry type.
///
/// Capability grants can be deleted like other entries.
/// Unlike most other entries, deleting a grant is linear, there is no branching tree of CRUD history because only grants on the local source chain can be deleted.
///
/// Once a capability grant is deleted any incoming function call requests with associated capability claims will immediately begin to fail as Unauthorized.
/// There is no `undo` for deletes, a new grant must be created and distributed to reinstate access after a grant deletion.
/// Immediately means after the wasm successfully completes with no errors or rollbacks as extern calls are transactional/atomic.
///
/// The input to delete_cap_grant is the HeaderHash of the CapGrant element to delete.
/// Deletes can reference both CapGrant creates and updates.
///
/// @see create_cap_grant
pub fn delete_cap_grant(hash: HeaderHash) -> ExternResult<HeaderHash> {
    delete(hash)
}

/// Generate secrets for capability grants.
///
/// Wraps the `random_bytes` HDK3 function with appropriate parameters set.
/// Generates 512 bits of cryptographic strength randomness to form the secret for a capability grant.
///
/// It is strongly recommended to always use this function for generating capability grant secrets.
/// There is negligible benefit to decreasing or increasing the bits of entropy, or changing the algorithm.
/// There may be security risks in shortening the secret or changing its generation logic.
///
/// Capability secrets must be unique within and across all chains.
/// Using this function consistently guarantees uniqueness.
///
/// If an attacker can guess a secret to masquerade as another agent and execute Unassigned code.
///
/// Re-using secrets is forbidden within and across all claims and grants.
pub fn generate_cap_secret() -> ExternResult<CapSecret> {
    random_bytes(CAP_SECRET_BYTES as u32).map(|bytes| {
        // Always a fatal error if our own bytes generation has the wrong number of bytes.
        assert_eq!(CAP_SECRET_BYTES, bytes.len());
        let mut inner = [0; CAP_SECRET_BYTES];
        inner.copy_from_slice(bytes.as_ref());
        CapSecret::from(inner)
    })
}

/// Update a capability secret.
///
/// Wraps the `update` HDK3 function with system type parameters set.
/// This guards against updating application entries or setting the wrong entry types.
///
/// Capability grant updates work exactly as a delete+create of the old+new grant entries.
///
/// The first argument is the header hash of the old grant being deleted as per `delete_cap_grant`.
/// The second argument is the entry value of the new grant to create as per `create_cap_grant`.
///
/// @see create_cap_grant
/// @see delete_cap_grant
pub fn update_cap_grant(
    old_grant_header_hash: HeaderHash,
    new_grant_value: CapGrantEntry,
) -> ExternResult<HeaderHash> {
    update(
        old_grant_header_hash,
        EntryWithDefId::new(EntryDefId::CapGrant, Entry::CapGrant(new_grant_value)),
    )
}
