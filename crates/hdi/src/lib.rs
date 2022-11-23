//! Holochain Deterministic Integrity (HDI) is Holochain's data model and integrity toolset for
//! writing zomes.
//!
//! The logic of a Holochain DNA can be divided into two parts: integrity and coordination.
//! Integrity is the part of the hApp that defines the data types and validates data
//! manipulations. Coordination encompasses the domain logic and implements the functions
//! that manipulate data.
//!
//! # Examples
//!
//! An example of an integrity zome with data definition and data validation can be found in the
//! wasm workspace of the Holochain repository:
//! <https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/integrity_zome/src/lib.rs>.
//!
//! # Data definition
//!
//! The DNA's data model is defined in integrity zomes. They comprise all data type definitions
//! as well as relationships between those types. Integrity zomes are purely definitions and do
//! not contain functions to manipulate the data. Therefore a hApp's data model is encapsulated
//! and completely independent of the domain logic, which is encoded in coordinator zomes.
//!
//! The MVC (model, view, controller) design pattern can be used as an analogy. **The
//! application’s integrity zomes comprise its model layer** — everything that defines the shape
//! of the data. In practice, this means three things:
//! - entry type definitions
//! - link type definitions
//! - a validation callback that constrains the kinds of data that can validly be called entries
//! and links of those types (see also [`validate`](prelude::validate)).
//!
//! **The coordination zomes comprise the application's controller layer** — the code that actually
//! writes and retrieves data, handles countersigning sessions and sends and receives messages
//! between peers or between a cell and its UI. In other words, all the zome functions, `init`
//! functions, remote signal receivers, and scheduler callbacks will all live in coordinator zomes.
//!
//! Advantages of this approach are:
//! * The DNA hash is constant as long as the integrity zomes remain the same. The peer network of
//! a DNA is tied to its hash. Changes to the DNA hash result in a new peer network. Changes to the
//! domain logic enclosed in coordinator zomes, however, do not affect the DNA hash. Hence the DNAs
//! and therefore hApps can be modified without creating a new peer network on every
//! deployment.
//! * Integrity zomes can be shared among DNAs. Any coordinator zome can import an integrity
//! zome's data types and implement functions for data manipulation. This composability of
//! integrity and coordinator zomes allows for a multitude of permutations with shared integrity
//! zomes, i. e. a shared data model.
//!
//! # Data validation
//!
//! The second fundamental part of integrity zomes is data validation. For every [operation](holochain_integrity_types::Op)
//! that can be performed on the data, a validation rule can be specified. Both data types and data
//! values can be validated. All of these validation rules are written in a central callback
//! which is called by the Holochain engine for each operation.
//!
//! There's a helper type called [`OpType`](holochain_integrity_types::OpType) available for easy
//! access to all link and entry variants when validating an operation. In many cases, this type can
//! be easier to work with than the bare [`Op`](holochain_integrity_types::Op), which contains the
//! same information as `OpType`, but the former has a flatter data structure, whereas the latter has
//! a deeply nested structure.
//!
//! ```
//! # #[cfg(not(feature = "test_utils"))]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # Ok(())
//! # }
//! # #[cfg(feature = "test_utils")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use hdi::prelude::*;
//! # #[hdk_entry_helper]
//! # pub struct A;
//! # #[hdk_entry_helper]
//! # pub struct B;
//! # #[hdk_entry_defs(skip_hdk_extern = true)]
//! # #[unit_enum(UnitEntryTypes)]
//! # pub enum EntryTypes {
//! #     A(A),
//! #     B(B),
//! # }
//! # #[hdk_link_types(skip_no_mangle = true)]
//! # pub enum LinkTypes {
//! #   A,
//! #   B,
//! # }
//! # let op = holochain_integrity_types::Op::RegisterCreateLink(
//! # holochain_integrity_types::RegisterCreateLink {
//! #     create_link: holochain_integrity_types::SignedHashed {
//! #         hashed: holo_hash::HoloHashed {
//! #             content: holochain_integrity_types::CreateLink {
//! #                 author: AgentPubKey::from_raw_36(vec![0u8; 36]),
//! #                 timestamp: Timestamp(0),
//! #                 action_seq: 1,
//! #                 prev_action: ActionHash::from_raw_36(vec![0u8; 36]),
//! #                 base_address: EntryHash::from_raw_36(vec![0u8; 36]).into(),
//! #                 target_address: EntryHash::from_raw_36(vec![0u8; 36]).into(),
//! #                 zome_index: 0.into(),
//! #                 link_type: 0.into(),
//! #                 tag: ().into(),
//! #                 weight: Default::default(),
//! #             },
//! #             hash: ActionHash::from_raw_36(vec![0u8; 36]),
//! #         },
//! #         signature: Signature([0u8; 64]),
//! #     },
//! # },
//! # );
//! # #[cfg(feature = "test_utils")]
//! # hdi::test_utils::set_zome_types(&[(0, 2)], &[(0, 2)]);
//! # let result: Result<hdi::prelude::ValidateCallbackResult, Box<dyn std::error::Error>> =
//! match op.to_type()? {
//!     OpType::StoreEntry(OpEntry::CreateEntry { entry_type, .. }) => match entry_type {
//!         EntryTypes::A(_) => Ok(ValidateCallbackResult::Valid),
//!         EntryTypes::B(_) => Ok(ValidateCallbackResult::Invalid(
//!             "No Bs allowed in this app".to_string(),
//!         )),
//!     },
//!     OpType::RegisterCreateLink {
//!         base_address: _,
//!         target_address: _,
//!         tag: _,
//!         link_type,
//!     } => match link_type {
//!         LinkTypes::A => Ok(ValidateCallbackResult::Valid),
//!         LinkTypes::B => Ok(ValidateCallbackResult::Invalid(
//!             "No Bs allowed in this app".to_string(),
//!         )),
//!     },
//!     _ => Ok(ValidateCallbackResult::Valid),
//! };
//! # Ok(())
//! # }
//! ```
//! See an example of the `validate` callback in an integrity zome in the WASM workspace:
//! <https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/validate/src/integrity.rs>.
//! Many more validation examples can be browsed in that very workspace.

/// Current HDI rust crate version.
pub const HDI_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use hdk_derive::hdk_entry_defs;
pub use hdk_derive::hdk_entry_helper;
pub use hdk_derive::hdk_extern;
pub use hdk_derive::hdk_link_types;

/// Working with app and system entries.
///
/// Most Holochain applications will define their own app entry types.
///
/// App entries are all entries that are not system entries.
/// Definitions of entry types belong in the integrity zomes of a DNA. In contrast, operations
/// for manipulating entries go into coordinator zomes.
///
/// # Examples
///
/// Refer to the WASM workspace in the Holochain repository for examples.
/// Here's a simple example of an entry definition:
/// <https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/entry_defs/src/integrity.rs>
///
/// An example of a coordinator zome with functions to manipulate entries:
/// <https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/coordinator_zome/src/lib.rs>
///
/// CRUD in Holochain is represented as a graph/tree of Records referencing each other (via Action hashes) representing new states of a shared identity.
/// Because the network is always subject to the possibility of partitions, there is no way to assert an objective truth about the 'current' or 'real' value that all participants will agree on.
/// This is a key difference between Holochain and blockchains.
/// Where blockchains define a consensus algorithm that brings all participants as close as possible to a single value while Holochain lets each participant discover their own truth.
///
/// The practical implication of this is that agents fetch as much information as they can from the network then follow an algorithm to 'walk' or 'reduce' the revisions and discover 'current' for themselves.
///
/// In Holochain terms, blockchain consensus is walking all the known 'updates' (blocks) that pass validation then walking/reducing down them to disover the 'chain with the most work' or similar.
/// For example, to implement a blockchain in Holochain, attach a proof of work to each update and then follow the updates with the most work to the end.
///
/// There are many other ways to discover the correct path through updates, for example a friendly game of chess between two players could involve consensual re-orgs or 'undos' of moves by countersigning a different update higher up the tree, to branch out a new revision history.
///
/// Two agents with the same information may even disagree on the 'correct' path through updates and this may be valid for a particular application.
/// For example, an agent could choose to 'block' another agent and ignore all their updates.
pub mod entry;

pub mod hash;

/// Maps a Rust function to an extern that WASM can expose to the Holochain host.
///
/// Annotate any compatible function with `#[hdk_extern]` to expose it to Holochain as a WASM extern.
/// The [`map_extern!`] macro is used internally by the `#[hdk_extern]` attribute.
///
/// Compatible functions:
///
/// - Have a globally unique name
/// - Accept `serde::Serialize + std::fmt::Debug` input
/// - Return `Result<O, WasmError>` (`ExternResult`) output where `O: serde::Serialize + std::fmt::Debug`
///
/// This module only defines macros so check the HDI crate root to see more documentation.
///
/// A _new_ extern function is created with the same name as the function with the `#[hdk_extern]` attribute.
/// The new extern is placed in a child module of the current scope.
/// This new extern is hoisted by WASM to share a global namespace with all other externs so names must be globally unique even if the base function is scoped.
///
/// The new extern handles:
///
/// - Extern syntax for Rust
/// - Receiving the serialized bytes from the host at a memory pointer specified by the guest
/// - Setting the HDI WASM tracing subscriber as the global default
/// - Deserializing the input from the host
/// - Calling the function annotated with `#[hdk_extern]`
/// - Serializing the result
/// - Converting the serialized result to a memory pointer for the host
/// - Error handling for all the above
///
/// If you want to do something different to the default you will need to understand and reimplement all the above.
pub mod map_extern;

/// Exports common types and functions according to the Rust prelude pattern.
pub mod prelude;

/// Encryption and decryption using the (secret)box algorithms popularised by Libsodium.
///
/// Libsodium defines and implements two encryption functions `secretbox` and `box`.
/// The former implements shared secret encryption and the latter does the same but with a DH key exchange to generate the shared secret.
/// This has the effect of being able to encrypt data so that only the intended recipient can read it.
/// This is also repudiable so both participants know the data must have been encrypted by the other (because they didn't encrypt it themselves) but cannot prove this to anybody else (because they _could have_ encrypted it themselves).
/// If repudiability is not something you want, you need to use a different approach.
///
/// Note that the secrets are located within the secure lair keystore (@todo actually secretbox puts the secret in WASM, but this will be fixed soon) and never touch WASM memory.
/// The WASM must provide either the public key for box or an opaque _reference_ to the secret key so that lair can encrypt or decrypt as required.
///
/// @todo implement a way to export/send an encrypted shared secret for a peer from lair
///
/// Note that even though the elliptic curve is the same as is used by ed25519, the keypairs cannot be shared because the curve is mathematically translated in the signing vs. encryption algorithms.
/// In theory the keypairs could also be translated to move between the two algorithms but Holochain doesn't offer a way to do this (yet?).
/// Create new keypairs for encryption and save the associated public key to your local source chain, and send it to peers you want to interact with.
pub mod x_salsa20_poly1305;

/// Rexporting the paste macro as it is used internally and may help structure downstream code.
pub use paste;

/// Create and verify signatures for serializable Rust structures and raw binary data.
///
/// The signatures are always created with the [Ed25519](https://en.wikipedia.org/wiki/EdDSA) algorithm by the secure keystore (lair).
///
/// Agent public keys that identify agents are the public half of a signing keypair.
/// The private half of the signing keypair never leaves the secure keystore and certainly never touches WASM.
///
/// If a signature is requested for a public key that has no corresponding private key in lair, the signing will fail.
///
/// Signatures can always be verified with the public key alone so can be done remotely (by other agents) and offline, etc.
///
/// The elliptic curve used by the signing algorithm is the same as the curve used by the encryption algorithms but is _not_ constant time (because signature verification doesn't need to be).
///
/// In general it is __not a good idea to reuse signing keys for encryption__ even if the curve is the same, without mathematically translating the keypair, and even then it's dubious to do so.
pub mod ed25519;

/// Request contextual information from the Holochain host.
///
/// The Holochain host has additional runtime context that the WASM may find useful and cannot produce for itself including:
///
/// - The calling agent
/// - The current app (bundle of DNAs)
/// - The current DNA
/// - The current Zome
/// - The function call itself
pub mod info;

#[cfg(feature = "trace")]
/// Integrates HDI with the Rust tracing crate.
///
/// The functions and structs in this module do _not_ need to be used directly.
/// The `#[hdk_extern]` attribute on functions exposed externally all set the `WasmSubscriber` as the global default.
///
/// This module defines a [ `trace::WasmSubscriber` ] that forwards all tracing macro calls to another subscriber on the host.
/// The logging level can be changed for the host at runtime using the `WASM_LOG` environment variable that works exactly as `RUST_LOG` for other tracing.
pub mod trace;

/// The interface between the host and guest is implemented as an `HdiT` trait.
///
/// The `set_hdi` function globally sets a `RefCell` to track the current HDI implementation.
/// When the `mock` feature is set then this will default to an HDI that always errors, else a WASM host is assumed to exist.
/// The `mockall` crate (in prelude with `mock` feature) can be used to generate compatible mocks for unit testing.
/// See mocking examples in the test WASMs crate, such as `agent_info`.
pub mod hdi;

pub mod link;

pub mod chain;

#[deny(missing_docs)]
pub mod op;

#[cfg(any(feature = "test_utils", test))]
pub mod test_utils;
