use crate::prelude::*;

/// Create a CapGrant on the grantor's chain.
///
/// When an agent wants to expose externs to be called remotely by other agents they need to select
/// a security model and probably generate a secret.
///
/// The input needs to evalute to a `ZomeCallCapGrant` struct which defines the tag, access and
/// granted zome/function pairs. The access is a `CapAccess` enum with variants `Unrestricted`,
/// `Transferable`, and `Assigned`.
///
/// The tag is an arbitrary `String` the developer or users can use to categorise and administer
/// grants committed to the chain. The tag should also match the `CapClaim` tags committed on the
/// recipient chain when a `CapGrant` is committed and shared.
///
/// Provided the grant author agent is online:
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
pub fn create_cap_grant(cap_grant_entry: CapGrantEntry) -> HdkResult<HeaderHash> {
    create(EntryWithDefId::new(
        EntryDefId::CapGrant,
        Entry::CapGrant(cap_grant_entry),
    ))
}
