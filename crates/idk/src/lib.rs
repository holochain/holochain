//! The Holochain Development Kit (IDK) provides high and low level functions for writing Holochain applications.
//!
//! Functions of a Holochain application (hApp) can be organized into reusable components. In Holochain terminology these components are called "zomes".
//! One or multiple zomes are compiled into a WebAssembly (WASM) binary, referred to as a DNA. All of the DNAs of an application are bundled to a hApp.
//!
//! hApps can be installed on a device that's running a so-called conductor, Holochain's runtime. Clients can then call each zome's functions via Remote Procedure Calls (RPC).
//! Holochain employs websocket ports for these RPCs, served by the conductor. Calls are made either from a client on localhost or from other nodes on the network.
//! The zome function to be executed must be specified in each call. Every zome function in turn defines the response it returns to the client as part of a zome's code.
//! [More info on Holochain's architecture](https://developer.holochain.org/concepts/2_application_architecture/)
//!
//! hApps are required to produce and validate data deterministically, which is stored in a content-addressable manner retrieved by hash value.
//! Since hApps are run as a binary on the hosting system, they must run in a sandboxed environment to prevent execution of insecure commands.
//! Instead of writing and maintaining a custom format and specification for these artifacts as well as a runtime environment to execute them,
//! Holochain makes use of WASM as the format of its applications. WASM binaries meet the aforementioned requirements as per the
//! [WebAssembly specification](https://webassembly.github.io/spec/core/).
//!
//! Low-level communication between the conductor and WASM binaries, like typing and serialization of data, is encapsulated by the IDK.
//! Using the IDK, hApp developers can focus on their application's logic. [Learn more about WASM in Holochain.](https://github.com/holochain/holochain/blob/develop/crates/hdk/ON-WASM.md)
//!
//! To start developing hApps, there's a [**hApp build tutorial**](https://github.com/holochain/happ-build-tutorial).
//!
//! # Example zomes 🍭
//!
//! The IDK is used in all the WASMs used to test Holochain itself.
//! As they are used directly by tests in CI they are guaranteed to compile and work for at least the tests we define against them.
//!
//! At the time of writing there were about 40 example/test WASMs that can be browsed [on github](https://github.com/holochain/holochain/tree/develop/crates/test_utils/wasm/wasm_workspace).
//!
//! Each example WASM is a minimal demonstration of specific IDK functionality, such as generating random data, creating entries or defining validation callbacks.
//! Some of the examples are very contrived, none are intended as production grade hApp examples, but do highlight key functionality.
//!
//!
//! ### IDK has layers 🧅
//!
//! IDK is designed in layers so that there is some kind of 80/20 rule.
//! The code is not strictly organised this way but you'll get a feel for it as you write your own hApps.
//!
//! Roughly speaking, 80% of your apps can be production ready using just 20% of the IDK features and code.
//! These are the 'high level' functions such as [ `crate::entry::create_entry` ] and macros like [ `#[hdk_extern]` ].
//! Every Holochain function is available with a typed and documented wrapper and there is a set of macros for exposing functions and defining entries.
//!
//! The 20% of the time that you need to go deeper there is another layer followng its own 80/20 rule.
//! 80% of the time you can fill the gaps from the layer above with `host_call` or by writing your own entry definition logic.
//! For example you may want to implement generic type interfaces or combinations of structs and enums for entries that isn't handled out of the box.
//!
//! If you need to go deeper still, the next layer is the `holochain_wasmer_guest`, `holochain_integrity_types` and `holochain_serialization` crates.
//! Here you can customise exactly how your externally facing functions are called and how they serialize data and memory.
//! Ideally you never need to go this far but there are rare situations that may require it.
//! For example, you may need to accept data from an external source that cannot be messagepack serialized (e.g. json), or you may want to customise the tracing tooling and error handling.
//!
//! The lowest layer is the structs and serialization that define how the host and the guest communicate.
//! You cannot change this but you can reimplement it in your language of choice (e.g. Haskell?) by referencing the Rust zome types and extern function signatures.
//!
//! > Note: From the perspective of hApp development in WASM the 'guest' is the WASM and the 'host' is the running Holochain conductor.
//! > The host is _not_ the 'host operating system' in this context.
//!
//!
//! ### IDK code structure 🧱
//!
//! IDK implements several key features:
//!
//! - Base IDKT trait for standardisation, mocking, unit testing support: hdk module
//! - Capabilities and function level access control: capability module
//! - Application data and entry definitions for the source chain and DHT: entry module and `entry_defs` callback
//! - Referencing/linking entries on the DHT together into a graph structure: link module
//! - Defining tree-like structures out of links and entries for discoverability and scalability: hash_path module
//! - Create, read, update, delete (CRUD) operations on the above
//! - Libsodium compatible symmetric/secret (secretbox) and asymmetric/keypair (box) encryption: x_salsa20_poly1305 module
//! - Ed25519 signing and verification of data: ed25519 module
//! - Exposing information about the current execution context such as zome name: info module
//! - Other utility functions provided by the host such as generating randomness and timestamps that are impossible in WASM: utility module
//! - Exposing functions to external processes and callbacks to the host: `#[hdk_extern]` and `map_extern!` macros
//! - Integration with the Rust [tracing](https://docs.rs/tracing/0.1.23/tracing/) crate
//! - Exposing a prelude of common types and functions for convenience
//!
//! Generally these features are structured logically into modules but there are some affordances to the layering of abstractions.
//!
//! ### IDK is based on callbacks 👂
//!
//! The only way to execute logic inside WASM is by having the host/conductor call a function that is marked as an 'extern' by the guest.
//!
//! Similarly, the only way for the guest to do anything other than process data and calculations is to call functions the host provides to the guest at runtime.
//!
//! The latter are all defined by the Holochain conductor and implemented by IDK for you, but the former need to all be defined by your application.
//! Any WASM that does _not_ use the IDK will need to define placeholders for and the interface to the host functions.
//!
//! All host functions can be called directly as:
//!
//! ```ignore
//! use crate::prelude::*;
//! let _output: IDK.with(|h| h.borrow().host_fn(input));
//! ```
//!
//! And every host function defined by Holochain has a convenience wrapper in IDK that does the type juggling for you.
//!
//! To extend a Rust function so that it can be called by the host, add the `#[hdk_extern]` attribute.
//!
//! - The function must take _one_ argument that implements `serde::Serialize + std::fmt::Debug`
//! - The function must return an `ExternResult` where the success value implements `serde::Serialize + std::fmt::Debug`
//! - The function must have a unique name across all externs as they share a global namespace in WASM
//! - Everything inside the function is Rust-as-usual including `?` to interact with `ExternResult` that fails as `WasmError`
//! - Use the `WasmError::Guest` variant for failure conditions that the host or external processes needs to be aware of
//! - Externed functions can be called as normal by other functions inside the same WASM
//!
//! For example:
//!
//! ```ignore
//! use crate::prelude::*;
//!
//! // This function can be called by any external process that can provide and accept messagepack serialized u32 integers.
//! #[hdk_extern]
//! pub fn increment(u: u32) -> ExternResult<u32> {
//!   Ok(u + 1)
//! }
//!
//! // Extern functions can be called as normal by other rust code.
//! assert_eq!(2, increment(1));
//! ```
//!
//! Most externs are simply available to external processes and must be called explicitly e.g. via RPC over websockets.
//! The external process only needs to ensure the input and output data is handled correctly as messagepack.
//!
//! Some externs function as callbacks the host will call at key points in Holochain internal system workflows.
//! These callbacks allow the guest to define how the host proceeds at key decision points.
//! Callbacks are simply called by name and they are 'sparse' in that they are matched incrementally from the most specific
//! name to the least specific name. For example, the `validate_{{ create|update|delete }}_{{ agent|entry }}` callbacks will
//! all match and all run during validation. All function components with muliple options are optional, e.g. `validate` will execute and so will `validate_create`.
//!
//! Holochain will merge multiple callback results for the same callback in a context sensitive manner. For example, the host will consider initialization failed if _any_ init callback fails.
//!
//! The callbacks are:
//!
//! - `fn entry_defs(_: ()) -> ExternResult<EntryDefs>`:
//!   - `EntryDefs` is a vector defining all entries used by this app.
//!   - The `entry_defs![]` macro simplifies this to something resembling `vec![]`.
//!   - The `#[hdk_entry]` attribute simplifies generating entry definitions for a struct or enum.
//!   - The `entry_def_index!` macro converts a def id like "post" to an `EntryDefIndex` by calling this callback _inside the guest_.
//!   - All zomes in a DNA define all their entries at the same time for the host
//!   - All entry defs are combined into a single ordered list per zone and exposed to tooling such as DNA generation
//!   - Entry defs are referenced by `u8` numerical position externally and in DHT headers and by id/name e.g. "post" in sparse callbacks
//! - `fn init(_: ()) -> ExternResult<InitCallbackResult>`:
//!   - Allows the guest to pass/fail/retry initialization with `InitCallbackResult`
//!   - All zomes in a DNA init at the same time
//!   - Any failure fails initialization for the DNA, any retry (missing dependencies) causes the DNA to retry
//!   - Failure overrides retry
//! - `fn migrate_agent_{{ open|close }} -> ExternResult<MigrateAgentCallbackResult>`:
//!   - Allows the guest to pass/fail a migration attempt to/from another DNA
//!   - Open runs when an agent is starting a new source chain from an old one
//!   - Close runs when an agent is deprecating an old source chain in favour of a new one
//!   - All zomes in a DNA migrate at the same time
//!   - Any failure fails the migration
//! - `fn post_commit(headers: Vec<SignedHeaderHashed>)`:
//!   - Executes after the WASM call that originated the commits so not bound by the original atomic transaction
//!   - Input is all the header hashes that were committed
//!   - The zome that originated the commits is called
//! - `fn validate_create_link(create_link_data: ValidateCreateLinkData) -> ExternResult<ValidateLinkCallbackResult>`:
//!   - Allows the guest to pass/fail/retry link creation validation
//!   - Only the zome that created the link is called
//! - `fn validate_delete_link(delete_link_data: ValidateDeleteLinkData) -> ExternResult<ValidateLinkCallbackResult>`:
//!   - Allows the guest to pass/fail/retry link deletion validation
//!   - Only the zome that deleted the link is called
//! - `fn validate_{{ create|update|delete }}_{{ agent|entry }}_{{ <entry_id> }}(validate_data: ValidateData) -> ExternResult<ValidateCallbackResult>`:
//!   - Allows the guest to pass/fail/retry entry validation
//!   - <entry_id> is the entry id defined by entry defs e.g. "comment"
//!   - Only the originating zome is called
//!   - Failure overrides retry
//! - `fn validation_package_{{ <entry_id> }}(entry_type: AppEntryType) -> ExternResult<ValidationPackageCallbackResult>`:
//!   - Allows the guest to build a validation package for the given entry type
//!   - Can pass/retry/fail/not-implemented in reverse override order
//!   - <entry_id> is the entry id defined by entry defs e.g. "comment"
//!   - Only the originating zome is called
//!
//! ### IDK is atomic on the source chain ⚛
//!
//! All writes to the source chain are atomic within a single extern/callback call.
//!
//! This means __all data will validate and be written together or nothing will__.
//!
//! There are no such guarantees for other side effects. Notably we cannot control anything over the network or outside the Holochain database.
//!
//! Remote calls will be atomic on the recipients device but could complete successfully while the local agent subsequently errors and rolls back their chain.
//! This means you should not rely on data existing _between_ agents unless you have another source of integrity such as cryptographic countersignatures.
//!
//! Use a post commit hook and signals or remote calls if you need to notify other agents about completed commits.
//!
//! ### IDK should be pinned 📌
//!
//! The basic functionality of the IDK is to communicate with the Holochain conductor using a specific typed interface.
//!
//! If any of the following change relative to the conductor your WASM _will_ have bugs:
//!
//! - Shared types used by the host and guest to communicate
//! - Serialization logic that generates bytes used by cryptographic algorithms
//! - Negotiating shared memory between the host and guest
//! - Functions available to be called by the guest on the host
//! - Callbacks the guest needs to provide to the host
//!
//! For this reason we have dedicated crates for serialization and memory handling that rarely change.
//! IDK references these crates with `=x.y.z` syntax in Cargo.toml to be explicit about this.
//!
//! IDK itself has a slower release cycle than the Holochain conductor by design to make it easier to pin and track changes.
//!
//! You should pin your dependency on IDK using the `=x.y.z` syntax too!
//!
//! You do _not_ need to pin _all_ your Rust dependencies, just those that take part in defining the host/guest interface.
//!
//! ### IDK is integrated with rust tracing for better debugging 🐛
//!
//! Every extern defined with the `#[hdk_extern]` attribute registers a [tracing subscriber](https://crates.io/crates/tracing-subscriber) that works in WASM.
//!
//! All the basic tracing macros `trace!`, `debug!`, `warn!`, `error!` are implemented.
//!
//! However, tracing spans currently do _not_ work, if you attempt to `#[instrument]` you will likely panic your WASM.
//!
//! WASM tracing can be filtered at runtime using the `WASM_LOG` environment variable that works exactly as `RUST_LOG` does for the Holochain conductor and other Rust binaries.
//!
//! The most common internal errors, such as invalid deserialization between WASM and external processes, are traced as `error!` by default.
//!
//! ### IDK requires explicit error handling between the guest and host ⚠
//!
//! All calls to functions provided by the host can fail to execute cleanly, at the least serialization could always fail.
//!
//! There are many other possibilities for failure, such as a corrupt database or attempting cryptographic operations without a key.
//!
//! When the host encounters a failure `Result` it will __serialize the error and pass it back to the WASM guest__.
//! The __guest must handle this error__ and either return it back to the host which _then_ rolls back writes (see above) or implement some kind of graceful failure or retry logic.
//!
//! The `Result` from the host in the case of host calls indicates whether the execution _completed_ successfully and is _in addition to_ other Result-like enums.
//! For example, a remote call can be `Ok` from the host's perspective but contain an [ `crate::prelude::ZomeCallResponse::Unauthorized` ] "failure" enum variant from the remote agent, both need to be handled in context.

/// Working with app and system entries.
///
/// Most Holochain applications will define their own app entry types.
///
/// App entries are all entries that are not system entries.
/// They are defined in the `entry_defs` callback and then the application can call CRUD functions with them.
///
/// CRUD in Holochain is represented as a graph/tree of Elements referencing each other (via Header hashes) representing new states of a shared identity.
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
/// The [ `map_extern!` ] macro is used internally by the `#[hdk_extern]` attribute.
///
/// Compatible functions:
///
/// - Have a globally unique name
/// - Accept `serde::Serialize + std::fmt::Debug` input
/// - Return `Result<O, WasmError>` (`ExternResult`) output where `O: serde::Serialize + std::fmt::Debug`
///
/// This module only defines macros so check the IDK crate root to see more documentation.
///
/// A _new_ extern function is created with the same name as the function with the `#[hdk_extern]` attribute.
/// The new extern is placed in a child module of the current scope.
/// This new extern is hoisted by WASM to share a global namespace with all other externs so names must be globally unique even if the base function is scoped.
///
/// The new extern handles:
///
/// - Extern syntax for Rust
/// - Receiving the serialized bytes from the host at a memory pointer specified by the guest
/// - Setting the IDK WASM tracing subscriber as the global default
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
/// Integrates IDK with the Rust tracing crate.
///
/// The functions and structs in this module do _not_ need to be used directly.
/// The `#[hdk_extern]` attribute on functions exposed externally all set the `WasmSubscriber` as the global default.
///
/// This module defines a [ `trace::WasmSubscriber` ] that forwards all tracing macro calls to another subscriber on the host.
/// The logging level can be changed for the host at runtime using the `WASM_LOG` environment variable that works exactly as `RUST_LOG` for other tracing.
pub mod trace;

/// The interface between the host and guest is implemented as an `HdkT` trait.
///
/// The `set_hdk` function globally sets a `RefCell` to track the current IDK implementation.
/// When the `mock` feature is set then this will default to an IDK that always errors, else a WASM host is assumed to exist.
/// The `mockall` crate (in prelude with `mock` feature) can be used to generate compatible mocks for unit testing.
/// See mocking examples in the test WASMs crate, such as `agent_info`.
pub mod idk;
