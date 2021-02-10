//! ## Holochain Development Kit 3.0 (HDK3)
//!
//! This is the third major iteration of the holochaind development kit.
//!
//! The HDK3 exists to make working with WASM in holochain much easier.
//!
//! Hopefully you dont' notice the WASMness at all and it just feels like Rust 🦀
//!
//! Note: From the perspective of happ development in WASM the 'guest' is the WASM and the 'host' is the running holochain conductor.
//! The host is _not_ the 'host operating system' in this context.
//!
//! ### HDK3 has layers 🧅
//!
//! HDK3 is designed in layers so that there is some kind of 80/20 rule.
//! The code is not strictly organised this way but you'll get a feel for it as you write your own happs.
//!
//! Roughly speaking, 80% of your apps can be production ready using just 20% of the HDK3 features and code.
//! These are the 'high level' functions such as `create_entry` and macros like `#[hdk_extern]`.
//! Every holochain function is available with a typed and documented wrapper and there is a set of macros for exposing functions and defining entries.
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
//! ### HDK3 should be pinned 📌
//!
//! The basic functionality of the HDK3 is to communicate with the holochain conductor using a specific typed interface.
//!
//! If any of the following change relative to the conductor your wasm _will_ have bugs:
//!
//! - Shared types used by the host and guest to communicate
//! - Serialization logic that generates bytes used by cryptographic algorithms
//! - Negotiating shared memory between the host and guest
//! - Functions available to be called by the guest on the host
//! - Callbacks the guest needs to provide to the host
//!
//! For this reason we have dedicated crates for serialization and memory handling that rarely change.
//! HDK3 references these crates with `=x.y.z` syntax in Cargo.toml to be explicit about this.
//!
//! HDK3 itself has a slower release cycle than the holochain conductor by design to make it easier to pin and track changes.
//!
//! You should pin your dependency on HDK3 using the `=x.y.z` syntax too!
//!
//! You do _not_ need to pin _all_ your Rust dependencies, just those that take part in defining the host/guest interface.
//!
//! ### HDK3 has many simple example zomes 🍭
//!
//! The HDK3 is used in all the wasms used to test holochain itself.
//! As they are used directly by tests in CI they are guaranteed to compile and work for at least the tests we define against them.
//!
//! At the time of writing there were about 40 example/test wasms that can be browsed [on github](https://github.com/holochain/holochain/tree/develop/crates/test_utils/wasm/wasm_workspace).
//!
//! Each example wasm is a minimal demonstration of specific HDK3 functionality, such as generating random data, creating entries or defining validation callbacks.
//! Some of the examples are very contrived, none are intended as production grade happ examples, but do highlight key functionality.
//!
//! ### HDK3 code structure 🧱
//!
//! HDK3 implements several key features:
//!
//! - Capabilities and function level access control: capability module
//! - Application data and entry definitions for the source chain and DHT: entry module and `entry_defs` callback
//! - Referencing/linking entries on the DHT together into a graph structure: link module
//! - Defining tree-like structures out of links and entries for discoverability and scalability: hash_path module
//! - Create, read, update, delete (CRUD) operations on the above
//! - Libsodium compatible symmetric/secret (secretbox) and asymmetric/keypair (box) encryption: x_salsa20_poly1305 module
//! - Ed25519 signing and verification of data: ed25519 module
//! - Exposing information about the current execution context such as zome name: info module
//! - Other utility functions provided by the host such as generating randomness and timestamps that are impossible in wasm: utility module
//! - Exposing functions to external processes and callbacks to the host: `#[hdk_extern]` and `map_extern!` macros
//! - Integration with the Rust [tracing](https://docs.rs/tracing/0.1.23/tracing/) crate
//! - Exposing a prelude of common types and functions for convenience
//!
//! Generally these features are structured logically into modules but there are some affordances to the layering of abstractions.
//!
//! ### HDK3 is based on callbacks 👂
//!
//! The only way to execute logic inside WASM is by having the host/conductor call a function that is marked as an 'extern' by the guest.
//!
//! Similarly, the only way for the guest to do anything other than process data and calculations is to call functions the host provides to the guest at runtime.
//!
//! The latter are all defined by the holochain conductor and implemented by HDK3 for you, but the former need to all be defined by your application.
//! Any wasm that does _not_ use the HDK3 will need to define placeholders for and the interface to the host functions.
//!
//! All host functions can be called directly as:
//!
//! ```rust
//! use crate::prelude::*;
//! let _output: ExternResult<OutputType> = host_call::<InputType, OutputType>(__host_extern_name, input);
//! ```
//!
//! And every host function defined by holochain has a convenience wrapper in HDK3 that does the type juggling for you.
//!
//! To extend a Rust function so that it can be called by the host, add the `#[hdk_extern]` attribute.
//!
//! - The function must take _one_ argument that implements `serde::Serialize + std::fmt::Debug`
//! - The function must return an `ExternResult` where the success value implements `serde::Serialize + std::fmt::Debug`
//! - The function must have a unique name across all externs as they share a global namespace in wasm
//! - Everything inside the function is Rust-as-usual including `?` to interact with `ExternResult` that fails as `WasmError`
//! - Use the `WasmError::Guest` variant for failure conditions that the host or external processes needs to be aware of
//! - Externed functions can be called as normal by other functions inside the same wasm
//!
//! For example:
//!
//! ```rust
//! use crate::prelude::*;
//!
//! // This function can be called by any external process that can provide and accept messagepack serialized u32 integers.
//! #[hdk_extern]
//! pub function increment(u: u32) -> ExternResult<u32> {
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
//! Some externs function as callbacks the host will call at key points in holochain internal system workflows.
//! These callbacks allow the guest to define how the host proceeds at key decision points.
//! Callbacks are simply called by name and they are 'sparse' in that they are matched incrementally from the most specific
//! name to the least specific name. For example, the `validate_{{ create|update|delete }}_{{ agent|entry }}` callbacks will
//! all match and all run during validation. All function components with muliple options are optional, e.g. `validate` will execute and so will `validate_create`.
//!
//! Holochain will merge multiple callback results for the same callback in a context sensitive manner. For example, the host will consider initialization failed if _any_ init callback fails.
//!
//! The callbacks are:
//!
//! - `function entry_defs(_: ()) -> ExternResult<EntryDefs>`:
//!   - `EntryDefs` is a vector defining all entries used by this app.
//!   - The `entry_defs![]` macro simplifies this to something resembling `vec![]`.
//!   - The `#[hdk_entry]` attribute simplifies generating entry definitions for a struct or enum.
//!   - The `entry_def_index!` macro converts a def id like "post" to an `EntryDefIndex` by calling this callback _inside the guest_.
//!   - All zomes in a DNA define all their entries at the same time for the host
//!   - All entry defs are combined into a single ordered list per zone and exposed to tooling such as DNA generation
//!   - Entry defs are referenced by `u8` numerical position externally and in DHT headers and by id/name e.g. "post" in sparse callbacks
//! - `function init(_: ()) -> ExternResult<InitResult>`:
//!   - Allows the guest to pass/fail/retry initialization with `InitResult`
//!   - All zomes in a DNA init at the same time
//!   - Any failure fails initialization for the DNA, any retry (missing dependencies) causes the DNA to retry
//!   - Failure overrides retry
//! - `function migrate_agent_{{ open|close }} -> ExternResult<MigrateAgentResult>`:
//!   - Allows the guest to pass/fail a migration attempt to/from another DNA
//!   - Open runs when an agent is starting a new source chain from an old one
//!   - Close runs when an agent is deprecating an old source chain in favour of a new one
//!   - All zomes in a DNA migrate at the same time
//!   - Any failure fails the migration
//! - `function post_commit(headers: Vec<HeaderHash>) -> ExternResult<PostCommitResult>`:
//!   - Allows the guest a final veto to entry commits or to perform side effects in response
//!   - Executes after the wasm call that originated the commits so not bound by the original atomic transaction
//!   - Guest is guaranteed that the commits will not be rolled back if Ok(PostCommitResult::Pass) is returned
//!   - Input is all the header hashes that were committed
//!   - Only the zome that originated the commits is called
//!   - Any failure fails (rolls back) all commits
//! - `function validate_create_link(create_link_data: ValidateCreateLinkData) -> ExternResult<ValidateLinkResult>`:
//!   - Allows the guest to pass/fail/retry link creation validation
//!   - Only the zome that created the link is called
//! - `function validate_delete_link(delete_link_data: ValidateDeleteLinkData) -> ExternResult<ValidateLinkResult>`:
//!   - Allows the guest to pass/fail/retry link deletion validation
//!   - Only the zome that deleted the link is called
//! - `function validate_{{ create|update|delete }}_{{ agent|entry }}_{{ <entry_id> }}(validate_data: ValidateData) -> ExternResult<ValidateResult>`:
//!   - Allows the guest to pass/fail/retry entry validation
//!   - <entry_id> is the entry id defined by entry defs e.g. "comment"
//!   - Only the originating zome is called
//!   - Failure overrides retry
//! - `function validation_package_{{ <entry_id> }}(entry_type: AppEntryType) -> ExternResult<ValidationPackageResult>`:
//!   - Allows the guest to build a validation package for the given entry type
//!   - Can pass/retry/fail/not-implemented in reverse override order
//!   - <entry_id> is the entry id defined by entry defs e.g. "comment"
//!   - Only the originating zome is called
//!
//! ### HDK3 is atomic on the source chain ⚛
//!
//! All writes to the source chain are atomic within a single extern/callback call.
//!
//! This means __all data will validate and be written together or nothing will__.
//!
//! There are no such guarantees for other side effects. Notably we cannot control anything over the network or outside the holochain database.
//!
//! Remote calls will be atomic on the recipients device but could complete successfully while the local agent subsequently errors and rolls back their chain.
//! This means you should not rely on data existing _between_ agents unless you have another source of integrity such as cryptographic countersignatures.
//!
//! Use a post commit hook and signals or remote calls if you need to notify other agents about completed commits.
//!
//! ### HDK3 is integrated with rust tracing for better debugging 🐛
//!
//! Every extern defined with the `#[hdk_extern]` attribute registers a [tracing subscriber](https://crates.io/crates/tracing-subscriber) that works in WASM.
//!
//! All the basic tracing macros `trace!`, `debug!`, `warn!`, `error!` are implemented.
//!
//! However, tracing spans currently do _not_ work, if you attempt to `#[instrument]` you will likely panic your WASM.
//!
//! WASM tracing can be filtered at runtime using the `WASM_LOG` environment variable that works exactly as `RUST_LOG` does for the holochain conductor and other Rust binaries.
//!
//! The most common internal errors, such as invalid deserialization between wasm and external processes, are traced as `error!` by default.
//!
//! ### HDK3 requires explicit error handling between the guest and host ⚠
//!
//! All calls to functions provided by the host can fail to execute cleanly, at the least serialization could always fail.
//!
//! There are many other possibilities for failure, such as a corrupt database or attempting cryptographic operations without a key.
//!
//! When the host encounters a failure `Result` it will __serialize the error and pass it back to the wasm guest__.
//! The __guest must handle this error__ and either return it back to the host which _then_ rolls back writes (see above) or implement some kind of graceful failure or retry logic.
//!
//! The `Result` from the host in the case of host calls indicates whether the execution _completed_ successfully and is _in addition to_ other Result-like enums.
//! For example, a remote call can be `Ok` from the host's perspective but contain an `Unauthorized` "failure" enum variant from the remote agent, both need to be handled in context.

pub mod capability;

/// Working with app entries.
///
/// Most holochain applications will define their own app entry types.
///
/// App entries are all entries that are not system entries.
/// They are defined in the `entry_defs` callback and then the application can call CRUD functions with them.
pub mod entry;
pub mod guest_callback;
pub mod hash_path;
pub mod host_fn;
pub mod map_extern;
pub mod prelude;
pub mod x_salsa20_poly1305;
pub use paste;
