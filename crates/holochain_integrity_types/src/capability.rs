//! Capability Grants and Claims
//!
//! This module provides a system for granting other agents access to a
//! capability of the Grantor's cell, in a fine-grained manner. A Claimant that
//! satisfies the grant may exercise the granted capability against the Grantor.
//!
//! A grant names the capability it authorizes, and authorizes nothing else:
//! - `Capability::ZomeCall`: call the granted zome functions on the Grantor's
//!   conductor.
//! - `Capability::DirectSignal`: send the Grantor a direct signal.
//!
//! Who may exercise a capability is controlled by the grant's
//! `GrantConstraint`, which comes with three possible degrees of access
//! control:
//! - Unrestricted: anybody can exercise this capability
//! - Transferable: a secret must be provided, but anybody with the secret may
//!   exercise the capability
//! - Assigned: Like Transferable, but there is a list of approved AgentPubKeys,
//!   and requests from any other agents are ignored.
//!
//! Capabilities are declared by a Grantor via a **`CapGrant`**, which is
//! committed to their source chain as a private entry. The Grantor then shares
//! the secret with the agents they mean to grant access to.
//!
//! Capabilities are exercised by other agents via a **`CapClaim`** which they
//! commit to their source chain as a private entry. This struct contains the
//! information needed to refer to the capability as well as the secret needed
//! to send to the Grantor.

mod access;
mod claim;
mod grant;
mod secret;

pub use access::*;
pub use claim::*;
pub use grant::*;
pub use secret::*;
