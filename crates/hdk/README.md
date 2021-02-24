# Holochain Development Kit (HDK)

This HDK is currently in flux, expect rapid changes.

There are two big differences between this kit and previous kits.

This kit:

1. The DSL is ergonomic and composable, so optional if you want more control
2. It is compatible with the cell-driven version of holochain
3. Differentiates between the holochain API/interface and "sugar" syntax

Old kits:

1. The DSL is monolithic and so a bit fragile off the beaten track
2. Only compatible with the redux version of holochain
3. Requires all holochain interactions to be routed through the HDK


## HDK API

Welcome to the HDK.
There are low-level macros and high-level functions to aid writing happs.

The intention is that most of the time most developers will use the high level
functions as they leverage the Rust type system better than macros can. This
allows for more useful compiler and IDE feedback loops.

## Examples

### map_extern!

```rust
use crate::prelude::*;

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct MyInput;

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct MyOutput(MyInput);

fn _foo(input: MyInput) -> Result<MyOutput, WasmError> {
  Ok(MyOutput(input))
}

map_extern!(foo, _foo);
```

### entry_def! & entry_defs

```rust
#[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
pub struct Foo;

#[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
pub struct Bar;

const FOO_ID: &str = "foo";
const BAR_ID: &str = "bar";

// Long version
entry_def!(Foo EntryDef {
    id: FOO_ID.into(),
    crdt_type: CrdtType,
    required_validations: RequiredValidations::default(),
    visibility: EntryVisibility::Public,
});

// Short version
entry_def!(Bar EntryDef {
    id: BAR_ID.into(),
    ..Default::default()
});

entry_defs!(vec![Foo::entry_def(), Bar::entry_def()]);
```

### create_entry, get, hash_entry, create_link, get_links, debug!

```rust
// Create your entry types
let foo = Foo;
let bar = Bar;
// Commit the entries
let _foo_header_hash = create_entry(foo.clone())?;
let _bar_header_hash = create_entry(bar.clone())?;
// Get the entry hash of each entry
let foo_entry_hash = hash_entry(foo)?;
let bar_entry_hash = hash_entry(bar)?;
// Link from foo (base) to bar (target)
let _link_add_header_hash = create_link(foo_entry_hash.clone(), bar_entry_hash)?;
// Get the links back
let links = get_links(foo_entry_hash)?;
// Print out the links
debug!(links);
```

### call_remote, zome_info, agent_info

```rust
// Get your agent key
let agent_pubkey = agent_info()?.agent_pubkey;
// Get the name of this zome
let zome_name = zome_info()?.zome_name;
// Call your friends foo function
let result: SerializedBytes = call_remote(
    my_friends_agent_pubkey,
    zome_name,
    "foo".to_string(),
    CapSecret::default(),
    MyInput.try_into()?
)?;
// Get their output
let output: MyOutput = result.try_into()?;
// Print their output
debug!(output);
```

### Direct Api Call
The above macros are convenience macros for calling the api but this
can also be done directly as follows:

```rust
// Commit foo
let foo_header_hash = create_entry(foo.clone())?;
// Call the api directly:
// Create the Entry from bar.
let entry = Entry::App(bar.clone().try_into()?);
// Call the update_entry host_fn directly
let _bar_header_hash = crate::host_fn!(
    __update,
    UpdateInput::new((bar.clone().into(), entry, foo_header_hash)),
    UpdateOutput
)?;
```

Current version: 0.0.1

## Composable concepts

One of the main design goals of this HDK is to make it composable.

The macros mostly just remove boilerplate that tends to obfuscate core holochain
concepts and allow for bugs to sneak in.

These macros are designed to be largely "mechanical" though, if you want to do
something a little bespoke then there is always a more verbose option to fall
back on.

This means the abstractions provided by the HDK are optional and composable so
a developer can opt in to only the functionality that is useful to their app.

The test wasms used by holochain core are written without the HDK.

The _mandatory_ wasm components are instead maintained in the `holochain-wasmer`
crate https://github.com/holochain/holochain-wasmer. These mandatory components
exist because there needs to be a basic protocol that holochain can implement
to co-ordinate memory and callbacks with the wasm.

The `holochain-wasmer` repository contains 3 main crates:

- `holochain_wasmer_common`: shared abstractions for both the host and guest
- `holochain_wasmer_host`: implements wasm for holochain itself
- `holochain_wasmer_guest`: abstractions for _you_ to write wasm with, that also
  power the HDK under the hood

It is important that it is possible to write minimal wasms that are compatible
with holochain without pulling in "the kitchen sink" of irrelevant Rust
dependencies or hiding so many details behind a DSL that developers really have
no idea what is going on and end up "cargo culting" solutions wholesale.

The `holochain-wasmer` crate has its own detailed documentation but relevant
high level details will be included here.

## Holochain overview

Holochain has several high-level components:

- A DHT network that shares, validates and stores data
- Wasm & DNA files that are executed to provide application specific logic
- A user-facing websockets interface that enables interactive clients
- The holochain binary that co-ordinates all these components

Depending on which component(s) you are working with, the key concepts and
documentation may look very different.

This documentation describes how to write wasm files that are compatible with
the holochain core binary.

If this is your first time writing wasm, or even Rust code, don't worry!

The ocean of wasm and Rust development is vast and deep, but you only need to
dip your toes in to effectively write wasm for holochain.

- Holochain core handles many of the tough edge-cases for you, like checking
  cryptographic proofs and detecting common "bad behaviour" on the network
- The HDK (holochain development kit) provides a DSL (domain specific language)
  to remove most or all boilerplate
- Most of the low-level wasm limitations have been abstracted away, so you can
  mostly just write vanilla rust, using all the standard language features
- Most of the advanced functionality in Rust is not required, there is
  little or no need for multithreading, channels, locks, complex traits,
  lifetimes, etc. etc.

Every holochain wasm works in the same basic way. The application developer
writes some Rust code using the functionality exposed by holochain. As long as
the rust code can be compiled to wasm and exposes the interface that holochain
expects, then holochain can run it to manage a p2p DHT network.

There are three things that make a wasm holochain-compatible:

- It must use only the host functionality that holochain provides
- It must expose callback functions that holochain expects
- Memory handling and (de)serialization must be compatible with holochain

### Holochain functionality

Holochain exposes a list of holochain-specific things that a wasm can do.

For detailed documentation of the full list, see the `core/ribosome` module
inside core, but some illustrative examples include:

- `emit_signal`: publish data to subscribed clients
- `encrypt` & `decrypt`: use the agent's keypair to encrypt and decrypt data
- `sign`: use the agent's keypair to sign some data
- `commit_entry`: save some data to the local source chain and broadcast it to
  the DHT to be redundantly validated and stored
- `get_entry`: retrieve some data from local or the network given its hash
- `create_link`: create graph style relationships (links) between entries
- `get_links`: retrive links between entries using the DHT as a graph database
- `send`: send data directly to a known peer on the network

This toolkit of functionality is available to the wasm as a list of "extern"
functions that are all injected into the wasm by holochain - i.e. these
functions are all provided by holochain to be used by every wasm.

All of this functionality is enabled on the wasm guest by the
`holochain_externs!` macro in the `holochain_wasmer_guest` crate. It just needs
to be called once somewhere in the wasm.

### Holochain interface

Holochain drives all of the wasms it has installed in the same way.

Internally holochain handles all the multi-threading, co-ordination between the
network and websocket RPC connections to any interactive client (e.g. like an
 electron app).

Whenever holochain reaches some point where it needs to execute application
specific logic it will call one of the functions in the wasm directly.

This is analagous to how standalone binaries in Rust (and other languages)
start by running the `main` function by convention.

It is also similar to how "handler" functions are configured in common
"serverless" platforms like AWS lambda.

There are broadly two types of exposed functions:

- well defined callbacks that specific functionality e.g. "validate this entry"
- arbitrary functions that handle serialized data from the interactive client

Both work in the same way technically but they have different responsibilities.
Callbacks extend holochain itself whereas other extern functions extend the
interactive client that is sending requests via. holochain.

All the extern functions are run in a newly built, sandboxed wasm instance, so
there are no long-running processes and it is not possible for callbacks to
interact with each other directly or share data in memory.

Because all functionality is based on simple, sandboxed callbacks, there is no
need for the application developer to handle threading or other complexities.
Even minor memory leaks are relatively harmless as the wasm memory is dropped
wholesale after every extern function call.

### Holochain serialization and memory

Due to wasm limitations (see below) holochain must implement some process to
share complex data types between the host and the guest.

The full process is documented in detail in the `holochain-wasmer` repository.

https://github.com/holochain/holochain-wasmer

In short, there are a few functions that the guest needs to expose to the host
that the host will use to request safe memory allocations and deallocations from
the guest.

This allows the host to repect the guest's own memory allocation logic, and so
provides support for alternative allocators.

Exposing these functions is as simple as calling the `holochain_externs!` macro
in the `holochain_wasmer_guest` crate.

Once the host and guest can share memory safely, they need to decide on a
serialization format that data can be shared across the wasm boundary as.

Holochain uses the messagepack serialization format as it has several benefits:

- It is not tied to the rust compiler
- It is reasonably fast and compact
- It supports binary data natively (e.g. JSON does not)
- It is reasonably human readable and can even be automatically JSONified

If you aren't familiar with `serde`, messagepack and/or
`holochain_serialized_bytes`. then it's worth at least skim reading the
documentation.

- Messagepack: https://msgpack.org/index.html
- Serde: https://github.com/serde-rs/serde
- Serde messagepack: https://github.com/3Hren/msgpack-rust
- Holochain serialized bytes: https://github.com/holochain/holochain-serialization/tree/develop/crates/holochain_serialized_bytes


## Wasm overview

The first thing to understand is what wasm is and how it works.

Wasm code is "web assembly" https://webassembly.org/.

It is designed to be embedded in other systems and to execute efficiently and
predictably across every environment it is embedded in.

To achieve this wasm makes almost no assuptions about the environment and is
very low level (i.e. it is "assembly code").

We will start with an overview of the limitations of wasm because it is
important to understand the constraints of the system. It may seem overwhelming
but we have created conventions and macros that makes working with holochain as
straightforward as any other Rust code.

### Hosted execution environment

Wasm runs inside some kind of host as a guest execution environment.

Wasm can perform pure mathematical calculations only. Anything that requires
a side effect like reading/writing to a database, file system, network,
terminal, browser, etc. requires the host to provide that functionality.

For example, a wasm game engine running in a web browser may want to render
pixels to an HTML canvas element. The wasm code has no access to anything in the
host so it will need to precalculate all the pixels and hand them off to the
host somehow.

The wasm host has full read/write access to the wasm's memory but the guest can
only access its own memory and return values from `extern` functions that the
host calls.

This means that the portability of wasm code is only as good as the host's
ability to drive the guest `extern` functions with the right data and execution
logic, and to provide all the functionality the guest is expecting.

For example, if a wasm guest is written to expect file system style system calls
to be possible it will fail to execute in a web browser.

Conversely, if the host expects the guest to expose certain functions and accept
certain data structures, then the wasm behaviour will be undefined or simply
crash if the guest does not meet these requirements.

__In short, wasm code is fundamentally driven through bidirectional callbacks.__

### No types or rich data

There are no types other than integers, floats and functions.

Wasm is missing even basic collections like lists or sets.  
There are no strings or utf-8 characters.  
Integers don't even define whether they are signed or unsigned!  
An "integer" is just a 32 or 64 bit block of binary data that signed and
unsigned operators like "addition" can do something with at runtime.

Some float operations, specifically those that involve the sign of operations
against `NaN` are non-deterministic in wasm, so it is often recommended to avoid
their usage entirely in p2p applications where determinism is important.

This effectively leaves us with just 3 types of data:

- functions
- 32 bit chunks of binary data called `i32`
- 64 bit chunks of binary data called `i64`

__Which is pragmatically just one type of data: "several bits of binary".__

### Bleeding edge technology

The wasm spec is still being developed.

Most language support is "coming soon".

__Rust is the only language with mature and official support for wasm.__

## How holochain uses wasm

The holochain binary acts as both a wasm host and a websockets server that
accepts incoming RPC style function calls and forwards them through to the wasm
guest. Any `extern` functions inside the guest wasm can be called this way via.
websockets and the holochain host can call `extern` functions itself to
provide "hook" style extension points for developers.

For example, when a holochain "DNA" is first installed it will reference one or
several wasm files. If any of these wasm files defines a function called `init`
then holochain will call each of the `init` functions one after the other in the
order they are referenced in the DNA config files.

Defining an extern function for holochain in Rust is straightforward:

- define a function as usual;
- with the `extern "C"` keywords to expose it to the host;
- the `#[no_mangle]` attribute to stop the compiler from renaming it;
- and `RemotePointer` arg/return values from the `holochain-wasmer-guest` crate.

Since we will want to accept and return more complex data than small chunks of
binary, there are a few macros such as `host_args!()` and `ret!()` that import
and export anything that can be serialized with `serde` from and to `GuestPtr`
values.

It's probably not clear how it is possible to "serialize" arbitrary data into a
single `u64` so these macros will be explained in more detail below.

Rather than diving into technical details, let's start with some illustrative
examples that demonstrates working with holochain-friendly wasm _without_ a HDK.

### Example: Minimal wasm that will run

All the core externs required for holochain to interact with this wasm are
defined by the `holochain_externs!()` macro.

Every callback is optional so defining the core externs is enough to compile
the wasm and have holochain install and run it.

```rust
holochain_wasmer_guest::holochain_externs!();
```

### Example: Minimal `init` callback

To implement an `init` callback we need a few things:

- the `holochain_externs!()` macro as above
- to define an `extern` called `init` as described above
- to use the `ret!()` macro with the `ExternOutput` type to return an
  `InitCallbackResult` enum, serialized into a `GuestPtr`

That last point is a little complex so I'll break it down.

The `ExternOutput` struct wraps `SerializedBytes` from the
`holochain_serialized_bytes` crate. Anything that can be serialized can be
sent back to the host through this struct and the `ret!` macro.

Internally `ret!()` serializes any compatible data to messagepack, puts it in
the wasm guest memory, tells the rust memory allocator _not_ to drop it
automatically and returns the location of it back to the host so the host can
copy it out of wasm memory into its own memory and work with it from there.

In the case of a known callback like `init` the host will attempt to deserialize
the inner value of `ExternOutput` because it is expecting the guest to return a
meaningful value from the callback.

All the callback return values have a simple naming convention
`FooCallbackResult` so `init` must return an `InitCallbackResult` enum.

In the case of a function called due to an external RPC request, the host will
simply send back whatever is inside `ExternOutput` as raw binary bytes.

All the callback input and output values are defined in `holochain_zome_types`.

```rust
use holochain_wasmer_guest::*;
use holochain_zome_types::init::InitCallbackResult;

holochain_externs!();

// tell the compiler not to "mangle" (rename) the function
#[no_mangle]
// note the `extern "C"` keywords
pub extern "C" fn init(_: GuestPtr) -> GuestPtr {

 // `ret!` will `return` anything `TryInto<SerializedBytes>` as a `GuestPtr`
 ret!(
  ExternOutput::new(
   // the host will allow this wasm to be installed if it returns `Pass`
   InitCallbackResult::Pass.try_into().unwrap()
  )
 );

}
```

### Example: Minimal "hello world" extern

A minimal functional wasm that implements a `hello_world` function that can be
called by the outside world, e.g. via. websockets RPC calls.

Our function won't take any inputs or do any real work, it will simply return
a `GuestPtr` to an empty placeholder `HelloWorld` struct.

The holochain host would return a messagepack-serialized representation of this
`HelloWorld` struct back to the guest via. websockets RPC.

In a real application this struct can be anything that round-trips through
`SerializedBytes` from the `holochain_serialized_bytes` crate. This means
anything compatible with the messagepack implementation for `serde`.

The easiest way to make a struct do this is by deriving the traits:

```rust
#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
struct Foo;
```

Note in the example the usage of `ret!()` and `ExternOutput` exactly as in the
`init` example.

In this case the custom `HelloWorld` struct will be ignored by the holochain
host and forwarded as messagepack serialized data back to the websockets RPC
caller. It is up to the developers of the wasm and the websockets client to
make sure the data in the structs is handled correctly. For example, a web-based
SDK for JavaScript would likely convert the messagepack data into JSON data and
maybe massage the raw data to be more idiomatic in its naming conventions for a
JavaScript context before exposing it to some GUI framework.

```rust
use holochain_wasmer_guest::*;

holochain_externs!();

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
// a real application would put meaningful data in this struct
struct HelloWorld;

// the same #[no_mangle] attribute and `extern "C"` keywords are used to expose
// functions to the external world as to implement callback functions
#[no_mangle]
pub extern "C" hello_world(_: GuestPtr) -> GuestPtr {
 ret!(
  ExternOutput(
   // the derived serialization traits make this work
   HelloWorld.try_into().unwrap()
  )
 );
}
```

### Example: Slightly less minimal - install, RPC, commit & validate

A typical wasm for a real application will expose `extern` functions to be RPC
called by websockets, which will internally call host functions, that in turn
trigger guest callbacks, before a final result is returned back over websockets.

The most obvious example of this is an `extern` function that commits an entry.

Entries must be validated by both the holochain host and the guest wasm before
they are finalized on the local source chain or broadcast to the DHT network.

In addition to the patterns we demonstrated above for externs and callbacks, we
also need to introduce the `host_call()!` and `host_args!()` macros.

The `host_call()` function works very similarly to the `ret!()` macro in that it
takes serializable data on the guest and sends it to the host as a `GuestPtr`.
The difference is that instead of causing the guest to `return`, this data is
the argument to a function that _executes and blocks immediately on the host_
and the result is deserialized back into the guest wasm automatically, inline
from a returned `GuestPtr`.

The `host_args!()` macro can be used _immediately at the start of a guest
function execution_ to attempt to deserialize the `GuestPtr` argument from the
host to a guest function. Internally the host is writing bytes directly to the
guest wasm memory and telling the guest memory allocator to leak these bytes.
The `host_args!()` macro relies on and cleans up this temporary memory leak so
always call it before anything else. Note that `host_args!()` will short circuit
and return early with an error similar to the `?` operator if deserialization
fails on the guest.

Note that both the `host_call()` function and the `host_args!()` macros rely on the guest to
correctly deserialize the values that the host is copying to the guest's memory.

So, before looking at the code, here is a diagram of how our example wasm would
interact with holochain and the outside world, from `init` through to an extern
function call, an entry commit and successful validation callback.

The `validation` callback is an example of specificity in callbacks.
The base callback is called `validate` so we can implement that and it will be
triggered for every entry that needs validation.
We can also define a more specific `validate_entry` callback that will only be
triggered for entries defined and committed by the current wasm - i.e. only
"app entries" and not system entries like agent keys.

![holochain wasm flow](https://thedavidmeister.keybase.pub/holochain/docs/hdk/images/wasm-flow.jpg)

So here is the wasm code for all that.

It consists of a few components, some new and some already demonstrated above:

- `holochain_externs!()` to enable the holochain host to run the wasm
- A `Png` struct to hold binary PNG data as `u8` bytes
- An `extern` function `save_image` that will be callable over websockets RPC
- A `host_call` to `__create` inside `save_image` to commit the image
- A `validate_entry` callback function implementation to validate the PNG
- Some basic validation logic to ensure the PNG is under 10mb
- Calling `host_args!()` in both externs to accept input
- Calling `ret!()` to return the validation callback result

`init` is optional, we can ignore it and that is the same as it always passing.

```rust
use holochain_wasmer_guest::*;

holochain_externs!();

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
// a simple new type to hold binary PNG data
struct Png([u8]);

// the websockets client can send raw binary bytes as the payload to this
// save_image function and the `remote_ptr` arg will point to the image data
#[no_mangle]
pub extern "C" fn save_image(remote_ptr: GuestPtr) -> GuestPtr {

 // the deserialization pattern mimics serde
 // we tell the compiler that we expect a `Png` type and `host_args!()` will
 // attempt to create one from the guest memory starting at `remote_ptr`
 let png: Png = host_args!(remote_ptr);

 // for this example we don't care about the result of commit entry
 // a real application should handle it
 //
 // the important bit for this example is that we use host_call() and that the
 // __create function on the host will enqueue a validation callback
 let _: CreateOutput = host_call(
  // note that all host functions from holochain start with prefix `__`
  __create,
  CreateInput::new(
   Entry::App(
    // this serializes the image into an Entry enum that the holochain host
    // knows what to do with
    png.try_into().unwrap()
   )
  )
 ).unwrap();
}

#[no_mangle]
// the validate_entry is a more specific variant of the base validate hook
// all the input and output values and behaviour are the same as the base hook
// but it will only be triggered for Entry::App variants rather than any Entry
// this is optional, the same logic can be implemented with the base `validate`
pub extern "C" fn validate_entry(remote_ptr: GuestPtr) -> GuestPtr {

 // ExternInput is the mirror of ExternOutput
 let input: ExternInput = host_args!(remote_ptr);

 // we ret! a GuestInput containing SerializedBytes of ValidateCallbackResult
 ret!(ExternOutput::new(
  // attempt to deserialize an Entry from the inner
  // SerializedBytes of the ExternInput struct
  match Entry::try_from(input.into_inner()) {
   Ok(Entry::App(serialized_bytes)) => {
    // we only have one entry type in this wasm so we can deserialize it
    // directly into a Png type
    // more complex apps should implement an enum with variants for the data
    // types needed
    let png: Png = serialized_bytes.try_into().unwrap();

    // let's cap the png size at 10mb
    // if this was a real app we might want to use an image processing crate to
    // attempt to validate that the image data is not corrupt
    if png.len() > 10_000_000 {
     ValidateCallbackResult::Invalid("The image is too big".to_string())
    } else {
     ValidateCallbackResult::Valid
    }
   },
   // technically this is reachable as the deserialization could fail for an
   // entry we are asked to validate incoming from the dht
   // for a local-only to example this _is_ unreachable because we
   // control the serialization locally and the specific `validate_entry`
   // callback has already pre-filtered inputs to the Entry::App variant only
   _ => unreachable!(),
 }));

}
```

### Example: Don't panic!

Panicking inside wasm is generally really bad. Even worse than panicking inside
vanilla Rust.

There are several reasons for this:

- The tooling for backtraces etc. is worse than native rust
- It opens the potential for malicious actors on the DHT to force your wasm to
  crash with bad data (the last example above would hit `unreachable!()` if
  undeserializable garbage data was received from the network)
- The type of errors handled in wasm callbacks should typically be able to be
  handled gracefully because we can simply hand errors back to the host

But so far all the examples have been littered with `unwrap()` and similar.

This is because all the extern functions can only accept and receive `GuestPtr`
data and we had no tools to handle `Result` or `?` type logic that is idiomatic
to Rust.

If you read the `holochain_serialized_bytes` documentation linked above, or have
worked with the crate before, you would know that we cannot simply serialize a
`Result` value directly into `SerializedBytes`. It needs to be wrapped in a
newtype struct first.

Even if we rewrote `SerializedBytes` to be compatible with `Result` it would not
help much because wasm doesn't allow us to return `Result` values from wasm
`extern` functions. This breaks the most common and useful Rust idiom for error
handling, the `?` operator.

To fill the `Result`/`?` gap we have two macros in `holochain_wasmer_guest`.

- `ret_err!()`: works like `ret!()` but accepts a failure string and is
  interpreted as an error value by the host
- `try_result!()`: works like `?` but uses `ret_err!()` under the hood

These macros are only needed inside `extern` functions and nothing requires that
all the app logic is limited to `extern` functions. Regular rust functions that
work with `Result` values directly can be written as normal, just not at the
point where callbacks cross the host/guest wasm boundary.

Note also that `host_args!()` does something _similar_ to `ret_err!()`
internally when it short-circuits in the case of failing to deserialize args.

The two most obvious cases for using `try_result!()` are:

- coupled with `host_call()` inside simple extern functions/callbacks
- to wrap vanilla rust code that returns a result to avoid logic-in-externs

This example will show a simple `host_call()` error handling but the next
example will show how to use all the macros together to collapse all the extern
logic into some generic, standalone boilerplate.

```rust
use holochain_wasmer_guest::*;

holochain_externs!();

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
struct Message(String);

#[no_mangle]
// an extern that allows websockets to commit some utf-8 message string
pub extern "C" fn commit_message(remote_ptr: GuestPtr) -> GuestPtr {
 let message: Message = host_args!(remote_ptr);
 // try_result! works exactly like `?`
 // it evalutes to v from Ok(v) or short circuits with Err(e)
 // because this is inside an extern function the short circuit logic also
 // handles memory and serialization logic for the holochain host
 let commit_entry_output: CreateOutput = try_result!(
  host_call(
   __create,
   CreateInput::new(
    Entry::App(
     try_result!(
      message.try_into(),
      "failed to serialize message to be committed"
     )
    )
   )
  ),
  "commit entry call failed"
 );
 ret!(
  ExternOutput(
   try_result!(
    commit_entry_output.try_into(),
    "failed to deserialize commit entry output"
   )
  )
 );
}
```

### Example: Quarantine extern boilerplate

While the wasmer macros do a lot of heavy lifting, they are still not as
ergonomic or idiomatic as vanilla rust would be.

Defining the externs and handling serialization, errors and memory correctly is
required but it only needs to be done once per extern.

Here is a combination of the last two examples, with less commentary but error
handling for the `Png` committing with minimal `try_result!()` calls.

At this point you should start to have a clear understanding of what the HDK is
doing under the hood and to decide for yourself whether you really need or want
the sugar that it provides.

The HDK macros simply expand to this extern boilerplate, saving you from typing
out a few macros to input/output data for the host. They also offer some
convenience wrappers around `host_call()` that do exactly what you'd expect,
e.g. `create_entry( ... )` vs. `host_call(__create, ... )`.

Think of the HDK as a tool and safety net but also don't feel you can't peek
under the hood to see what is there.

```rust
use holochain_wasmer_guest::*;

///////////////////////////////
// WASM BOILERPLATE STARTS HERE
///////////////////////////////

holochain_externs!();

#[no_mangle]
// note the generic structure of this extern...
pub extern "C" fn save_image(remote_ptr: GuestPtr) -> GuestPtr {
 ret!(try_result!(_save_image(host_args!(remote_ptr)), "save_image error"));
}

#[no_mangle]
// the generic structure of this extern matches save_image
pub extern "C" fn validate_entry(remote_ptr: GuestPtr) -> GuestPtr {
 ret!(try_result!(_validate_entry(host_args!(remote_ptr)), "validate entry error"));
}

/////////////////////////////
// WASM BOILERPLATE ENDS HERE
/////////////////////////////

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
struct Png([u8]);

// everything in here is "normal" rust
// no need for special keywords on the function
// the input args and return values are all native Rust types, not pointers
// we can use `Result` and `?`
// the only wasm-ey thing here is the `host_call()` macro
fn _save_image(png: Png) -> Result<CreateOutput, String> {
 Ok(host_call(
  __create,
  &CreateInput::new(
   Entry::App(
    png.try_into()?
   )
  )
 )?)
}

// absolutely nothing wasm specific here at all
// only need to know to return a ExternOutput with inner ValidateCallbackResult
fn _validate_entry(input: ExternInput) -> Result<ExternOutput, String> {
 Ok(ExternOutput(match Entry::try_from(input.into_inner())? {
  Entry::App(serialized_bytes) => {
   let png: Png = serialized_bytes.try_into()?;

   if png.len() > 10_000_000 {
    ValidateCallbackResult::Invalid("The image is too big".to_string())
   } else {
    ValiateCallbackResult::Valid
   }
  },
  // this really _is_ unreachable now
  // the specific validate_entry callback guards against other entry variants
  // all the fallible logic is guarded by `?`
  _ => unreachable!(),
 }.try_into()?))
}
```
