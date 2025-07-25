
# holochain_state

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Discord](https://img.shields.io/badge/Discord-blue.svg?style=flat-square)](https://discord.gg/k55DS5dmPH)
[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)

[![Crate](https://img.shields.io/crates/v/holochain_state.svg)](https://crates.io/crates/holochain_state)
[![API Docs](https://docs.rs/holochain_state/badge.svg)](https://docs.rs/holochain_state)

<!-- cargo-rdme start -->

The Holochain state crate provides helpers and abstractions for working
with the `holochain_sqlite` crate.

### Reads
The main abstraction for creating data read queries is the `Query` trait.
This can be implemented to make constructing complex queries easier.

The [`source_chain`] module provides the `SourceChain` type,
which is the abstraction for working with chains of actions.

The [`host_fn_workspace`] module provides abstractions for reading data during workflows.

### Writes
The [`mutations`] module is the complete set of functions
for writing data to sqlite in holochain.

### In-memory
The [`scratch`] module provides the `Scratch` type for
reading and writing data in memory that is not visible anywhere else.

The SourceChain type uses the Scratch for in-memory operations which
can be flushed to the database.

The Query trait allows combining arbitrary database SQL queries with
the scratch space so reads can union across the database and in-memory data.

<!-- cargo-rdme end -->
