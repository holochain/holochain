//! The Holochain Development Kit (HDK) provides high and low level functions for writing Holochain applications.
//!
//! Holochain is built as a client-server architecture. The Conductor, Holochain's runtime, acts as the server.
//! Its [Conductor API](https://docs.rs/holochain_conductor_api/latest/holochain_conductor_api) can be queried
//! by a client to manage hApps and send requests to hApp functions.
//! [Read more on Holochain's architecture.](https://developer.holochain.org/concepts/2_application_architecture)
//!
//! Functions of a hApp are organized into reusable components. In Holochain terminology these components are called "zomes".
//! One or multiple zomes are compiled into a WebAssembly (WASM) binary, referred to as a DNA. All of the DNAs of an application are bundled to a hApp.
//! In short, the structure is __hApp -> DNA -> zome -> function__.
//!
//! hApps can be developed using the HDK. See the [Holochain Quick Start Guide](https://developer.holochain.org/quick-start)
//! to get started with hApp development.
//!
//! # Example zomes 🍭
//!
//! There are numerous example/test WASMs on many aspects of hApp development that can be browsed
//! [on Github](https://github.com/holochain/holochain/tree/develop/crates/test_utils/wasm/wasm_workspace).
//!
//! Each example WASM is a minimal demonstration of specific HDK functionality, such as generating random data, creating entries or defining validation callbacks.
//! Some of the examples are very contrived, none are intended as production grade hApp examples, but do highlight key functionality.
//!
//! # Zomes are separated into data model and domain logic
//!
//! hApps are required to produce and validate data deterministically. There's a data model and a domain logic part to each hApp. In Holochain, the
//! data model is defined in integrity zomes and the domain logic is written in coordinator zomes.
//!
//! ## Integrity zomes 📐
//!
//! Integrity zomes describe a hApp's domain model by defining a set of entry and link types and providing a validation callback
//! function that checks the integrity of any operations that manipulate data of those types.
//! Additionally, a genesis self-check callback can be implemented for basic verification
//! of the data that allows an agent to join a network before they attempt to join it.
//!
//! The wasm workspace contains examples of integrity zomes like this:
//! <https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/integrity_zome/src/lib.rs>
//!
//! Refer to the [HDI crate](crate::prelude::hdi) for more information on the integrity layer.
//!
//! ## Coordinator zomes 🐜
//!
//! Coordinator zomes are the counterpart of integrity zomes in a DNA. They contain the domain logic of how data is read and written.
//! Whereas data is defined and validated in integrity zomes, functions to manipulate data are implemented in coordinator zomes.
//!
//! An example coordinator zome can be found in the wasm workspace of the Holochain repository:
//! <https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/coordinator_zome/src/lib.rs>.
//!
//! # HDK structure 🧱
//!
//! HDK implements several key features:
//!
//! - Base HDKT trait for standardisation, mocking, unit testing support: [`hdk`] module
//! - Capabilities and function level access control: [`capability`](crate::capability) module
//! - [Holochain Deterministic Integrity (HDI)](crate::hdi)
//! - Application data and entry definitions for the source chain and DHT: [`entry`](crate::entry)
//! module and [`entry_defs`](crate::prelude::entry_defs) callback
//! - Referencing/linking entries on the DHT together into a graph structure: [`link`](crate::link) module
//! - Defining tree-like structures out of links and entries for discoverability and scalability: [`hash_path`](crate::hash_path) module
//! - Create, read, update, delete (CRUD) operations on the above
//! - Libsodium compatible symmetric/secret (secretbox) and asymmetric/keypair (box) encryption: [`x_salsa20_poly1305`](crate::x_salsa20_poly1305) module
//! - Ed25519 signing and verification of data: [`ed25519`](crate::ed25519) module
//! - Exposing information about the current execution context such as zome name: [`info`](crate::info) module
//! - Other utility functions provided by the host such as generating randomness and timestamps that are impossible in WASM: utility module
//! - Exposing functions to external processes and callbacks to the host: [`hdk_extern!`](macro@crate::prelude::hdk_extern) and [`map_extern!`](macro@crate::prelude::map_extern) macros
//! - Integration with the Rust [tracing](https://docs.rs/tracing/0.1.23/tracing/) crate
//! - Exposing a [`prelude`](crate::prelude) of common types and functions for convenience
//!
//! Generally these features are structured logically into modules but there are some affordances to the layering of abstractions.
//!
//!
//! # HDK is based on callbacks 👂
//!
//! The only way to execute logic inside WASM is by having the conductor (host) call a function that is marked as an `extern` by the zome (guest).
//!
//! > Note: From the perspective of hApp development in WASM, the "guest" is the WASM and the "host" is the running Holochain conductor.
//! The host is _not_ the "host operating system" in this context.
//!
//! Similarly, the only way for the guest to do anything other than process data and calculations is to call functions the host provides to it at runtime.
//!
//! Host functions are all defined by the Holochain conductor and implemented by HDK for you, but the guest functions need to all be defined by your application.
//!
//! > Any WASM that does _not_ use the HDK will need to define placeholders for and the interface to the host functions.
//!
//! All host functions can be called directly as:
//!
//! ```ignore
//! use crate::prelude::*;
//! let _output: HDK.with(|h| h.borrow().host_fn(input));
//! ```
//!
//! And every host function defined by Holochain has a convenience wrapper in HDK that does the type juggling for you.
//!
//! Low-level communication between the conductor and WASM binaries, like typing and serialization of data, is abstracted by the HDK.
//! Using the HDK, hApp developers can focus on their application's logic. [Learn more about WASM in Holochain.](https://github.com/holochain/holochain/blob/develop/crates/hdk/ON-WASM.md)
//!
//! ## Extern callbacks = Zome functions
//!
//! To extend a Rust function so that it can be called by the host, add the [`hdk_extern!`](macro@crate::prelude::hdk_extern) attribute.
//!
//! - The function must take _one_ argument that implements `serde::Serialize + std::fmt::Debug`
//! - The function must return an `ExternResult` where the success value implements `serde::Serialize + std::fmt::Debug`
//! - The function must have a unique name across all externs as they share a global namespace in WASM
//! - Everything inside the function is Rust-as-usual including `?` to interact with `ExternResult` that fails as `WasmError`
//! - Use the [`wasm_error!`](crate::prelude::wasm_error) macro along with the
//! [`WasmErrorInner::Guest`](crate::prelude::WasmErrorInner::Guest) variant for failure conditions that the host or
//! external processes need to be aware of
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
//! ## Internal callbacks
//!
//! Some externs act as callbacks the host will call at key points in Holochain internal system workflows.
//! These callbacks allow the guest to define how the host proceeds at those decision points.
//! Callbacks are simply called by name and they are "sparse" in that they are matched incrementally from the most specific
//! name to the least specific name. For example, the `validate_{{ create|update|delete }}_{{ agent|entry }}` callbacks will
//! all match and all run during validation. All function components with multiple options are optional, e.g. `validate` will execute and so will `validate_create`.
//!
//! Holochain will merge multiple callback results for the same callback in a context sensitive manner. For example, the host will consider initialization failed if _any_ init callback fails.
//!
//! The callbacks are (see above for examples):
//!
//! - `fn entry_defs(_: ()) -> ExternResult<EntryDefsCallbackResult>`:
//!   - Typically implemented automatically by macros in the HDK so does NOT
//!     require writing the extern for it manually.
//!   - `EntryDefs` is a vector defining all entries used by this app.
//!   - All zomes in a DNA define all their entries at the same time for the host.
//!   - All entry defs are combined into a single ordered list per zome and exposed to tooling such as DNA generation.
//!   - Entry defs are referenced by `u8` numerical position externally and in DHT actions, and by id/name e.g. "post" in sparse callbacks.
//! - `fn genesis_self_check(_: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult>`:
//!   - Allows each agent to validate itself before attempting to join the
//!     network.
//!   - Receives `GenesisSelfCheckData` that includes DNA information, the agent
//!     key for the candidate source chain and the membrane proof.
//!   - Runs _before the agent exists on the network_ so has no ability to use
//!     the network and generally only has access to deterministic HDK functions.
//! - `fn init(_: ()) -> ExternResult<InitCallbackResult>`:
//!   - Allows the guest to pass/fail/retry initialization with [`InitCallbackResult`](crate::prelude::holochain_zome_types::init::InitCallbackResult).
//!   - Lazy execution - only runs when any zome of the DNA is first called.
//!   - All zomes in a DNA init at the same time.
//!   - Any zome failure fails initialization for the DNA, any zome retry (missing dependencies) causes the DNA to retry.
//!   - Failure overrides retry.
//!   - See [`create_cap_grant`](crate::capability::create_cap_grant) for an explanation of how to set up capabilities in `init`.
//! - `fn migrate_agent_{{ open|close }} -> ExternResult<MigrateAgentCallbackResult>`:
//!   - Allows the guest to pass/fail a migration attempt to/from another DNA.
//!   - Open runs when an agent is starting a new source chain from an old one.
//!   - Close runs when an agent is deprecating an old source chain in favour of a new one.
//!   - All zomes in a DNA migrate at the same time.
//!   - Any failure fails the migration.
//! - `fn post_commit(actions: Vec<SignedActionHashed>)`:
//!   - Executes after the WASM call that originated the commits so not bound by the original atomic transaction.
//!   - Input is all the action hashes that were committed.
//!   - The zome that originated the commits is called.
//! - `fn validate(op: Op) -> ExternResult<ValidateCallbackResult>`:
//!   - Allows the guest to pass/fail/retry any operation.
//!   - Only the originating zome is called.
//!   - Failure overrides retry.
//!   - See [`validate`](crate::hdi::prelude::validate) for more details.
//!
//! # HDK has layers 🧅
//!
//! HDK is designed in layers so that there is some kind of 80/20 rule.
//! The code is not strictly organised this way but you'll get a feel for it as you write your own hApps.
//!
//! Roughly speaking, 80% of your apps can be production ready using just 20% of the HDK features and code.
//! These are the 'high level' functions such as [`crate::entry::create_entry`] and macros like [`hdk_extern!`](macro@crate::prelude::hdk_extern).
//! Every Holochain function is available with a typed and documented wrapper and there is a set of macros for exposing functions and defining entries.
//!
//! The 20% of the time that you need to go deeper there is another layer followng its own 80/20 rule.
//! 80% of the time you can fill the gaps from the layer above with `host_call` or by writing your own entry definition logic.
//! For example you may want to implement generic type interfaces or combinations of structs and enums for entries that isn't handled out of the box.
//!
//! If you need to go deeper still, the next layer is the `holochain_wasmer_guest`, `holochain_zome_types` and `holochain_serialization` crates.
//! Here you can customise exactly how your externally facing functions are called and how they serialize data and memory.
//! Ideally you never need to go this far but there are rare situations that may require it.
//! For example, you may need to accept data from an external source that cannot be messagepack serialized (e.g. json), or you may want to customise the tracing tooling and error handling.
//!
//! The lowest layer is the structs and serialization that define how the host and the guest communicate.
//! You cannot change this but you can reimplement it in your language of choice (e.g. Haskell?) by referencing the Rust zome types and extern function signatures.
//!
//!
//! # HDK is atomic on the source chain ⚛
//!
//! [Read up on what the source chain is in Holochain.](https://developer.holochain.org/concepts/3_source_chain)
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
//!
//! # HDK should be pinned 📌
//!
//! The basic functionality of the HDK is to communicate with the Holochain conductor using a specific typed interface.
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
//! HDK references these crates with `=x.y.z` syntax in Cargo.toml to be explicit about this.
//!
//! HDK itself has a slower release cycle than the Holochain conductor by design to make it easier to pin and track changes.
//!
//! You should pin your dependency on HDK using the `=x.y.z` syntax too!
//!
//! You do _not_ need to pin _all_ your Rust dependencies, just those that take part in defining the host/guest interface.
//!
//!
//! # HDK is integrated with rust tracing for better debugging 🐛
//!
//! Every extern defined with the [`hdk_extern!`](macro@crate::prelude::hdk_extern) attribute registers a [tracing subscriber](https://crates.io/crates/tracing-subscriber) that works in WASM.
//!
//! All the basic tracing macros `trace!`, `debug!`, `warn!`, `error!` are implemented.
//!
//! However, tracing spans currently do _not_ work, if you attempt to `#[instrument]`, you will likely panic your WASM.
//!
//! WASM tracing can be filtered at runtime using the `WASM_LOG` environment variable that works exactly as `RUST_LOG` does for the Holochain conductor and other Rust binaries.
//!
//! The most common internal errors, such as invalid deserialization between WASM and external processes, are traced as `error!` by default.
//!
//!
//! # HDK requires explicit error handling between the guest and host ⚠
//!
//! All calls to functions provided by the host can fail to execute cleanly, at the least serialization could always fail.
//!
//! There are many other possibilities for failure, such as a corrupt database or attempting cryptographic operations without a key.
//!
//! When the host encounters a failure `Result`, it will __serialize the error and pass it back to the WASM guest__.
//! The __guest must handle this error__ and either return it back to the host which _then_ rolls back writes (see above), or implement some kind of graceful failure or retry logic.
//!
//! The `Result` from the host in the case of host calls indicates whether the execution _completed_ successfully and is _in addition to_ other Result-like enums.
//! For example, a remote call can be `Ok` from the host's perspective but contain an
//! [`ZomeCallResponse::Unauthorized`](crate::prelude::ZomeCallResponse::Unauthorized) "failure" enum variant from the remote agent.
//! Both need to be handled in context.

pub use hdi::HDI_VERSION;

/// Current HDK rust crate version.
pub const HDK_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
const ERAND_INTERNAL: u32 = getrandom::Error::CUSTOM_START + 1;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
const ERAND_TOO_LONG: u32 = getrandom::Error::CUSTOM_START + 2;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn wasm_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    if buf.len() > u32::MAX as usize {
        let code = core::num::NonZeroU32::new(ERAND_TOO_LONG).unwrap();
        return Err(getrandom::Error::from(code));
    }
    let number_of_bytes = buf.len() as u32;
    match crate::hdk::HDK.with(|h| h.borrow().random_bytes(number_of_bytes)) {
        Err(_) => {
            let code = core::num::NonZeroU32::new(ERAND_INTERNAL).unwrap();
            return Err(getrandom::Error::from(code));
        }
        Ok(bytes) => {
            if bytes.len() != buf.len() {
                let code = core::num::NonZeroU32::new(ERAND_INTERNAL).unwrap();
                return Err(getrandom::Error::from(code));
            }
            buf.copy_from_slice(&bytes);
        }
    }
    Ok(())
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
getrandom::register_custom_getrandom!(wasm_getrandom);

/// Capability claims and grants.
///
/// Every exposed function in Holochain uses capability grants/claims to secure
/// access.Capability grants are system entries committed to the source chain
/// that define access. Capability claims are system entries that reference a
/// grant on a source chain.
///
/// # Examples
/// <https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/capability/src/coordinator.rs>
///
/// 0. When Alice wants Bob to be able to call a function on her running conductor she commits a grant for Bob.
/// 0. Bob commits the grant as a claim on his source chain.
/// 0. When Bob wants to call Alice's function he sends the claim back to Alice along with the function call information.
/// 0. Alice cross references Bob's claim against her grant, e.g. to check it is still valid, before granting access.
///
/// There are four types of capability grant:
///
/// - Author: The author of the local source chain provides their agent key as a claim and has full access to all functions.
/// - Unrestricted: Anyone can call this function without providing a claim.
/// - Unassigned: Anyone with the randomly generated secret associated with the grant can call this function.
/// - Assigned: The specific named agents can call this function if they provide the associated secret.
///
/// Capability grants and claims reference each other by a shared, unique, unpredictable secret.
/// The security properties of a capability secret are roughly the same as an API key for a server.
///
/// - If an attacker knows or guesses the secret they can call Unassigned functions
/// - An attacker cannot call Assigned functions even if they know or guess the secret
/// - If a secret is compromised the grant can be deleted and new claims can be distributed
/// - The secret only grants access to live function calls against a running conductor reachable on the network
/// - Holochain compares capability secrets using constant time equality checks to mitigate timing attacks
/// - Grant secrets are stored in WASM memory so are NOT as secure as a dedicated keystore
///
/// Grant secrets are less sensitive than cryptographic keys but are not intended to be public data.
/// Don't store them to the DHT in plaintext, or commit them to Github repositories, etc!
///
/// For best security, assign grants to specific agents if you can as the assignment check _does_ cryptographically validate the caller.
///
// @todo in the future grant secrets may be moved to lair somehow.
pub mod capability;

/// Signing a single chain entry between multiple participants.
///
/// The basic goal is to enable a kind of atomicity across multiple source chains
/// in an environment where countersigners trust each other in some ways but not
/// entirely. Countersigning provides several trust models, including nominating
/// a single party to gather signatures, M of N signers, majority signing buckets,
/// etc.
///
/// The integrity layer enforces very little other than the structure of a
/// countersigned entry, to define the session parameters and uniqueness and final
/// signature set. Implementations are expected to drive countersigning sessions
/// through coordinator zomes based on understanding both the expected network
/// topologies and trust between peers on the network.
///
/// As various models for driving and finalising systems on the network are
/// defined and implemented they all end up in the countersigning crate.
///
/// This is a network level implementation of countersigning which has pros and
/// cons. There are also cryptographic methods of countersigning such as
/// threshold signatures that produce a single proof between multiple
/// participants, which are NOT included in this crate.
pub mod countersigning;

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
/// # CRUD
///
/// CRUD in Holochain is represented as a graph/tree of Records referencing each other (via Action hashes), representing new states of a shared identity.
/// Because the network is always subject to the possibility of partitions, there is no way to assert an objective truth about the 'current' or 'real' value that all participants will agree on.
/// This is a key difference between Holochain and blockchains.
/// Where blockchains define a consensus algorithm that brings all participants as close as possible to a single value, while Holochain lets each participant discover their own truth.
///
/// The practical implication of this is that agents fetch as much information as they can from the network, then follow an algorithm to 'walk' or 'reduce' the revisions and discover 'current' for themselves.
///
/// In Holochain terms, blockchain consensus is walking all the known 'updates' (blocks) that pass validation, then walking/reducing down them to disover the 'chain with the most work' or similar.
/// For example, to implement a blockchain in Holochain, attach a proof of work to each update and then follow the updates with the most work to the end.
///
/// There are many other ways to discover the correct path through updates, for example a friendly game of chess between two players could involve consensual re-orgs or 'undos' of moves by countersigning a different update higher up the tree, to branch out a new revision history.
///
/// Two agents with the same information may even disagree on the 'correct' path through updates and this may be valid for a particular application.
/// For example, an agent could choose to 'block' another agent and ignore all their updates.
pub mod entry;

pub use hdi;
pub use hdi::entry_defs;

pub mod hash;

/// Distributed Hash Tables (DHTs) are fundamentally all key/value stores (content addressable).
///
/// This has lots of benefits but can make discoverability difficult.
///
/// When agents have the hash for some content they can directly fetch it but they need a way to discover the hash.
/// For example, Alice can create new usernames or chat messages while Bob is offline.
/// Unless there is a registry at a known location for Bob to lookup new usernames and chat messages he will never discover them.
///
/// The most basic solution is to create a single entry with constant content, e.g. "chat-messages" and link all messages from this.
///
/// The basic solution has two main issues:
///
/// - Fetching _all_ chat messages may be something like fetching _all_ tweets (impossible, too much data)
/// - Holochain neighbourhoods (who needs to hold the data) center around the content address so the poor nodes closest to "chat-messages" will be forced to hold _all_ messages (DHT hotspots)
///
/// To address this problem we can introduce a tree structure.
/// Ideally the tree structure embeds some domain specific _granularity_ into each "hop".
/// For example the root level for chat messages could link to years, each year can link to months, then days and minutes.
/// The "minutes" level will link to all chat messages in that exact minute.
/// Any minutes with no chat messages will simply never be linked to.
/// A GUI can poll from as deep in the tree as makes sense, for example it could start at the current day when the application first loads and then poll the past 5 minutes in parallel every 2 minutes (just a conceptual example).
///
/// If the tree embeds granularity then it can replace the need for 'pagination' which is a problematic concept in a partitioned p2p network.
/// If the tree cannot embed meaningful granularity, for example maybe the only option is to build a tree based on the binary representation of the hash of the content, then we solve DHT hotspots but our applications will have no way to narrow down polling, other than to brute force the tree.
///
/// Examples of granularity include:
///
/// - Latitude/longitude for geo data
/// - Timestamps
/// - Lexical (alphabetical) ordering
/// - Orders of magnitude
/// - File system paths
/// - Etc.
///
/// When modelling your data into open sets/collections that need to be looked up, try to find a way to create buckets of granularity that don't need to be brute forced.
///
/// In the case that granularity can be defined the tree structure solves both our main issues:
///
/// - We never need to fetch _all_ messages because we can start as deeply down the tree as is appropriate and
/// - We avoid DHT hotspots because each branch of the tree has its own hash and set of links, therefore a different neighbourhood of agents
///
/// The [`hash_path`] module includes 3 submodules to help build and navigate these tree structures efficiently:
///
/// - [`hash_path::path`] is the basic general purpose implementation of tree structures as `Vec<Vec<u8>>`
/// - [`hash_path::shard`] is a string based DSL for creating lexical shards out of strings as utf-32 (e.g. usernames)
/// - [`hash_path::anchor`] implements the "anchor" pattern (two level string based tree, "type" and "text") in terms of paths
pub mod hash_path;

/// Maps a Rust function to an extern that WASM can expose to the Holochain host.
///
/// Annotate any compatible function with [`hdk_extern!`] to expose it to Holochain as a WASM extern.
/// The [`map_extern!`](crate::map_extern!) macro is used internally by the [`hdk_extern!`] attribute.
///
/// Compatible functions:
///
/// - Have a globally unique name
/// - Accept `serde::Serialize + std::fmt::Debug` input
/// - Return `Result<O, WasmError>` (`ExternResult`) output where `O: serde::Serialize + std::fmt::Debug`
///
/// This module only defines macros so check the HDK crate root to see more documentation.
///
/// A _new_ extern function is created with the same name as the function with the [`hdk_extern!`] attribute.
/// The new extern is placed in a child module of the current scope.
/// This new extern is hoisted by WASM to share a global namespace with all other externs so names must be globally unique even if the base function is scoped.
///
/// The new extern handles:
///
/// - Extern syntax for Rust
/// - Receiving the serialized bytes from the host at a memory pointer specified by the guest
/// - Setting the HDK WASM tracing subscriber as the global default
/// - Deserializing the input from the host
/// - Calling the function annotated with [`hdk_extern!`]
/// - Serializing the result
/// - Converting the serialized result to a memory pointer for the host
/// - Error handling for all the above
///
/// If you want to do something different to the default you will need to understand and reimplement all the above.
///
/// [`hdk_extern!`]: hdk_derive::hdk_extern
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
//
// @todo implement a way to export/send an encrypted shared secret for a peer from lair
///
/// Note that even though the elliptic curve is the same as is used by ed25519, the keypairs cannot be shared because the curve is mathematically translated in the signing vs. encryption algorithms.
/// In theory the keypairs could also be translated to move between the two algorithms but Holochain doesn't offer a way to do this (yet?).
/// Create new keypairs for encryption and save the associated public key to your local source chain, and send it to peers you want to interact with.
pub mod x_salsa20_poly1305;

/// Rexporting the paste macro as it is used internally and may help structure downstream code.
pub use paste;

/// Tools to interrogate source chains.
///
/// Interacting with a source chain is very different to the DHT.
///
/// - Source chains have a linear history guaranteed by action hashes
/// - Source chains have a single owner/author signing every chain record
/// - Source chains can be iterated over from most recent back to genesis by following the action hashes as references
/// - Source chains contain interspersed system and application entries
/// - Source chains contain both private (local only) and public (broadcast to DHT) records
///
/// There is a small DSL provided by `query` that allows for inspecting the current agent's local source chain.
/// Typically it will be faster, more direct and efficient to query local data than dial out to the network.
/// It is also possible to query local private entries.
///
/// Agent activity for any other agent on the network can be fetched.
/// The agent activity is _only the actions_ of the remote agent's source chain.
/// Agent activity allows efficient building of the history of an agent.
/// Agent activity is retrieved from a dedicated neighbourhood near the agent.
/// The agent's neighbourhood also maintains a passive security net that guards against attempted chain forks and/or rollbacks.
/// The same query DSL for local chain queries is used to filter remote agent activity actions.
pub mod chain;

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

/// Links in Holochain are analogous to a join table in a traditional SQL schema.
///
/// Links embody navigable graph structures between entires in a more general format than CRUD trees.
///
/// At a high level:
///
/// - Can implement direct or indirect circular references
/// - Reference data by its hash
/// - Have a base and target entry, action or external hash
/// - Can either exist or be deleted (i.e. there is no revision history, deleting removes a link permanently)
/// - Many links can point from/to the same hash
/// - Links reference entry hashes not actions
///
/// Links are retrived from the DHT by performing [ `link::get_links` ] or [ `link::get_link_details` ] against the _base_ of a link.
///
/// Links also support short (about 500 bytes) binary data to encode contextual data on a domain specific basis.
///
/// __Links are not entries__, there is only an action with no associated entry, so links cannot reference other links or maintain or participate in a revision history.
pub mod link;

/// Methods for interacting with peers in the same DHT network.
///
/// Data on the DHT generally propagates at the speed of gossip and must be explicitly polled and retrieved.
///
/// Often we want more responsive and direct interactions between peers.
/// These interactions come in two flavours, RPC style function calls and notification style 'signals'.
///
/// All function calls use capability grants and claims to authenticate and authorize.
/// Signals simply forward information about the introduction of new data on the DHT so that agents can push updates to each other rather than relying purely on polling.
//
// @todo introduce a pubsub mechanism
pub mod p2p;

/// Integrates HDK with the Rust tracing crate.
///
/// The functions and structs in this module do _not_ need to be used directly.
/// The [`hdk_extern!`] attribute on functions exposed externally all set the `WasmSubscriber` as the global default.
///
/// This module defines a [ `trace::WasmSubscriber` ] that forwards all tracing macro calls to another subscriber on the host.
/// The logging level can be changed for the host at runtime using the `WASM_LOG` environment variable that works exactly as `RUST_LOG` for other tracing.
///
/// [`hdk_extern!`]: hdk_derive::hdk_extern
pub mod trace;

/// Everything related to inspecting or responding to time.
///
/// Currently only fetching the host's opinion of the local time is supported.
//
// @todo implement scheduled execution and sleeping
pub mod time;

/// Generate cryptographic strength random data
///
/// The host provides the random bytes because any/all WASM implementations of randomness is flawed and insecure.
pub mod random;

/// The interface between the host and guest is implemented as an `HdkT` trait.
///
/// The `set_hdk` function globally sets a `RefCell` to track the current HDK implementation.
/// When the `mock` feature is set then this will default to an HDK that always errors, else a WASM host is assumed to exist.
/// The `mockall` crate (in prelude with `mock` feature) can be used to generate compatible mocks for unit testing.
/// See mocking examples in the test WASMs crate, such as `agent_info`.
pub mod hdk;
