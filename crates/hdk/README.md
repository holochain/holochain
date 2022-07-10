# hdk

The Holochain Development Kit (HDK) provides high and low level functions for writing Holochain applications.

Functions of a Holochain application (hApp) can be organized into reusable components. In Holochain terminology these components are called "zomes".
One or multiple zomes are compiled into a WebAssembly (WASM) binary, referred to as a DNA. All of the DNAs of an application are bundled to a hApp.
In short, that structure is __hApp -> DNA -> zome -> function__.

hApps are required to produce and validate data deterministically. There's a data model and a domain logic part to each hApp. In Holochain, the
data model is defined in integrity zomes and the domain logic is written in coordinator zomes. See Integrity zomes and Coordinator zomes further down and
[Holochain Deterministic Integrity](holochain_deterministic_integrity) for more information.

Since hApps are run as a binary on the hosting system, they must be sandboxed to prevent execution of insecure commands.
Instead of writing and maintaining a custom format and specification for these artifacts as well as a runtime environment to execute them,
Holochain makes use of WASM as the format of its applications. WASM binaries meet the aforementioned requirements as per the
[WebAssembly specification](https://webassembly.github.io/spec/core).

hApps can be installed on a device that's running a so-called conductor, Holochain's runtime. Clients can then call each zome's functions via Remote Procedure Calls (RPC).
Holochain employs websocket ports for these RPCs, served by the conductor. Calls are made either from a client on localhost or from other nodes on the network.
The zome function to be executed must be specified in each call. Every zome function defines the response it returns to the client.
[More info on Holochain's architecture](https://developer.holochain.org/concepts/2_application_architecture)

Low-level communication between the conductor and WASM binaries, like typing and serialization of data, is encapsulated by the HDK.
Using the HDK, hApp developers can focus on their application's logic. [Learn more about WASM in Holochain.](https://github.com/holochain/holochain/blob/develop/crates/hdk/ON-WASM.md)

See the [Holochain Learning Resources](https://developer.holochain.org/learning) to get started with hApp development.

## Example zomes 🍭

The HDK is used in all the WASMs used to test Holochain itself.
As they are used directly by tests in CI they are guaranteed to compile and work for at least the tests we define against them.

At the time of writing there were about 40 example/test WASMs that can be browsed
[on Github](https://github.com/holochain/holochain/tree/develop/crates/test_utils/wasm/wasm_workspace).

Each example WASM is a minimal demonstration of specific HDK functionality, such as generating random data, creating entries or defining validation callbacks.
Some of the examples are very contrived, none are intended as production grade hApp examples, but do highlight key functionality.

## Integrity zomes 📐

Integrity zomes describe a hApp's domain model by defining a set of entry and link types and providing a validation callback
function that checks the integrity of any operations that manipulate data of those types.

The wasm workspace contains examples of integrity zomes:
<https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/integrity_zome/src/lib.rs>

## Coordinator zomes 🐜

Coordinator zomes are the counterpart of integrity zomes in a DNA. They contain the domain logic of how data is read and written.
Whereas data is defined and validated in integrity zomes, functions to manipulate data are implemented in coordinator zomes.

An example coordinator zome can be found in the wasm workspace of the Holochain repository:
<https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/coordinator_zome/src/lib.rs>.

## HDK structure 🧱

HDK implements several key features:

- Base HDKT trait for standardisation, mocking, unit testing support: [`hdk`] module
- Capabilities and function level access control: [`capability`] module
- [Holochain Deterministic Integrity (HDI)](holochain_deterministic_integrity)
- Application data and entry definitions for the source chain and DHT: [`entry`] module and [`entry_defs`] callback
- Referencing/linking entries on the DHT together into a graph structure: [`link`] module
- Defining tree-like structures out of links and entries for discoverability and scalability: [`hash_path`] module
- Create, read, update, delete (CRUD) operations on the above
- Libsodium compatible symmetric/secret (secretbox) and asymmetric/keypair (box) encryption: [`x_salsa20_poly1305`] module
- Ed25519 signing and verification of data: [`ed25519`] module
- Exposing information about the current execution context such as zome name: [`info`] module
- Other utility functions provided by the host such as generating randomness and timestamps that are impossible in WASM: utility module
- Exposing functions to external processes and callbacks to the host: [`hdk_extern!`] and [`map_extern!`] macros
- Integration with the Rust [tracing](https://docs.rs/tracing/0.1.23/tracing/) crate
- Exposing a [`prelude`] of common types and functions for convenience

Generally these features are structured logically into modules but there are some affordances to the layering of abstractions.


## HDK is based on callbacks 👂

The only way to execute logic inside WASM is by having the host (conductor) call a function that is marked as an `extern` by the guest (WASM).

> Note: From the perspective of hApp development in WASM, the "guest" is the WASM and the "host" is the running Holochain conductor.
The host is _not_ the "host operating system" in this context.

Similarly, the only way for the guest to do anything other than process data and calculations is to call functions the host provides to the guest at runtime.

The latter are all defined by the Holochain conductor and implemented by HDK for you, but the former need to all be defined by your application.

> Any WASM that does _not_ use the HDK will need to define placeholders for and the interface to the host functions.

All host functions can be called directly as:

```rust
use crate::prelude::*;
let _output: HDK.with(|h| h.borrow().host_fn(input));
```

And every host function defined by Holochain has a convenience wrapper in HDK that does the type juggling for you.

### Extern callbacks

To extend a Rust function so that it can be called by the host, add the [`hdk_extern!`] attribute.

- The function must take _one_ argument that implements `serde::Serialize + std::fmt::Debug`
- The function must return an `ExternResult` where the success value implements `serde::Serialize + std::fmt::Debug`
- The function must have a unique name across all externs as they share a global namespace in WASM
- Everything inside the function is Rust-as-usual including `?` to interact with `ExternResult` that fails as `WasmError`
- Use the `WasmErrorInner::Guest` variant for failure conditions that the host or external processes needs to be aware of
- Externed functions can be called as normal by other functions inside the same WASM

For example:

```rust
use crate::prelude::*;

// This function can be called by any external process that can provide and accept messagepack serialized u32 integers.
#[hdk_extern]
pub fn increment(u: u32) -> ExternResult<u32> {
  Ok(u + 1)
}

// Extern functions can be called as normal by other rust code.
assert_eq!(2, increment(1));
```

Most externs are simply available to external processes and must be called explicitly e.g. via RPC over websockets.
The external process only needs to ensure the input and output data is handled correctly as messagepack.

### Internal callbacks

Some externs function as callbacks the host will call at key points in Holochain internal system workflows.
These callbacks allow the guest to define how the host proceeds at those decision points.
Callbacks are simply called by name and they are "sparse" in that they are matched incrementally from the most specific
name to the least specific name. For example, the `validate_{{ create|update|delete }}_{{ agent|entry }}` callbacks will
all match and all run during validation. All function components with multiple options are optional, e.g. `validate` will execute and so will `validate_create`.

Holochain will merge multiple callback results for the same callback in a context sensitive manner. For example, the host will consider initialization failed if _any_ init callback fails.

The callbacks are:

- [`fn entry_defs(_: ()) -> ExternResult<EntryDefs>`](entry_defs):
  - `EntryDefs` is a vector defining all entries used by this app.
  - All zomes in a DNA define all their entries at the same time for the host.
  - All entry defs are combined into a single ordered list per zome and exposed to tooling such as DNA generation.
  - Entry defs are referenced by `u8` numerical position externally and in DHT actions, and by id/name e.g. "post" in sparse callbacks.
- `fn init(_: ()) -> ExternResult<InitCallbackResult>`:
  - Allows the guest to pass/fail/retry initialization with [`InitCallbackResult`](holochain_zome_types::init::InitCallbackResult).
  - Lazy execution - only runs when any zome of the DNA is first called.
  - All zomes in a DNA init at the same time.
  - Any zome failure fails initialization for the DNA, any zome retry (missing dependencies) causes the DNA to retry.
  - Failure overrides retry.
  - See [`create_cap_grant`](crate::capability::create_cap_grant) for an explanation of how to set up capabilities in `init`.
- `fn migrate_agent_{{ open|close }} -> ExternResult<MigrateAgentCallbackResult>`:
  - Allows the guest to pass/fail a migration attempt to/from another DNA.
  - Open runs when an agent is starting a new source chain from an old one.
  - Close runs when an agent is deprecating an old source chain in favour of a new one.
  - All zomes in a DNA migrate at the same time.
  - Any failure fails the migration.
- `fn post_commit(actions: Vec<SignedActionHashed>)`:
  - Executes after the WASM call that originated the commits so not bound by the original atomic transaction.
  - Input is all the action hashes that were committed.
  - The zome that originated the commits is called.
- `fn validate_create_link(create_link_data: ValidateCreateLinkData) -> ExternResult<ValidateLinkCallbackResult>`:
  - Allows the guest to pass/fail/retry link creation validation.
  - Only the zome that created the link is called.
- `fn validate_delete_link(delete_link_data: ValidateDeleteLinkData) -> ExternResult<ValidateLinkCallbackResult>`:
  - Allows the guest to pass/fail/retry link deletion validation.
  - Only the zome that deleted the link is called.
- `fn validate(op: Op) -> ExternResult<ValidateCallbackResult>`:
  - Allows the guest to pass/fail/retry any operation.
  - Only the originating zome is called.
  - Failure overrides retry.
  - See [`validate`](holochain_deterministic_integrity::prelude::validate) for more details.

## HDK has layers 🧅

HDK is designed in layers so that there is some kind of 80/20 rule.
The code is not strictly organised this way but you'll get a feel for it as you write your own hApps.

Roughly speaking, 80% of your apps can be production ready using just 20% of the HDK features and code.
These are the 'high level' functions such as [`crate::entry::create_entry`] and macros like [`hdk_extern!`].
Every Holochain function is available with a typed and documented wrapper and there is a set of macros for exposing functions and defining entries.

The 20% of the time that you need to go deeper there is another layer followng its own 80/20 rule.
80% of the time you can fill the gaps from the layer above with `host_call` or by writing your own entry definition logic.
For example you may want to implement generic type interfaces or combinations of structs and enums for entries that isn't handled out of the box.

If you need to go deeper still, the next layer is the `holochain_wasmer_guest`, `holochain_zome_types` and `holochain_serialization` crates.
Here you can customise exactly how your externally facing functions are called and how they serialize data and memory.
Ideally you never need to go this far but there are rare situations that may require it.
For example, you may need to accept data from an external source that cannot be messagepack serialized (e.g. json), or you may want to customise the tracing tooling and error handling.

The lowest layer is the structs and serialization that define how the host and the guest communicate.
You cannot change this but you can reimplement it in your language of choice (e.g. Haskell?) by referencing the Rust zome types and extern function signatures.


## HDK is atomic on the source chain ⚛

All writes to the source chain are atomic within a single extern/callback call.

This means __all data will validate and be written together or nothing will__.

There are no such guarantees for other side effects. Notably we cannot control anything over the network or outside the Holochain database.

Remote calls will be atomic on the recipients device but could complete successfully while the local agent subsequently errors and rolls back their chain.
This means you should not rely on data existing _between_ agents unless you have another source of integrity such as cryptographic countersignatures.

Use a post commit hook and signals or remote calls if you need to notify other agents about completed commits.


## HDK should be pinned 📌

The basic functionality of the HDK is to communicate with the Holochain conductor using a specific typed interface.

If any of the following change relative to the conductor your WASM _will_ have bugs:

- Shared types used by the host and guest to communicate
- Serialization logic that generates bytes used by cryptographic algorithms
- Negotiating shared memory between the host and guest
- Functions available to be called by the guest on the host
- Callbacks the guest needs to provide to the host

For this reason we have dedicated crates for serialization and memory handling that rarely change.
HDK references these crates with `=x.y.z` syntax in Cargo.toml to be explicit about this.

HDK itself has a slower release cycle than the Holochain conductor by design to make it easier to pin and track changes.

You should pin your dependency on HDK using the `=x.y.z` syntax too!

You do _not_ need to pin _all_ your Rust dependencies, just those that take part in defining the host/guest interface.


## HDK is integrated with rust tracing for better debugging 🐛

Every extern defined with the [`hdk_extern!`] attribute registers a [tracing subscriber](https://crates.io/crates/tracing-subscriber) that works in WASM.

All the basic tracing macros `trace!`, `debug!`, `warn!`, `error!` are implemented.

However, tracing spans currently do _not_ work, if you attempt to `#[instrument]` you will likely panic your WASM.

WASM tracing can be filtered at runtime using the `WASM_LOG` environment variable that works exactly as `RUST_LOG` does for the Holochain conductor and other Rust binaries.

The most common internal errors, such as invalid deserialization between WASM and external processes, are traced as `error!` by default.


## HDK requires explicit error handling between the guest and host ⚠

All calls to functions provided by the host can fail to execute cleanly, at the least serialization could always fail.

There are many other possibilities for failure, such as a corrupt database or attempting cryptographic operations without a key.

When the host encounters a failure `Result` it will __serialize the error and pass it back to the WASM guest__.
The __guest must handle this error__ and either return it back to the host which _then_ rolls back writes (see above) or implement some kind of graceful failure or retry logic.

The `Result` from the host in the case of host calls indicates whether the execution _completed_ successfully and is _in addition to_ other Result-like enums.
For example, a remote call can be `Ok` from the host's perspective but contain an [ `crate::prelude::ZomeCallResponse::Unauthorized` ] "failure" enum variant from the remote agent, both need to be handled in context.

[`hdk_extern!`]: hdk_derive::hdk_extern

License: CAL-1.0
