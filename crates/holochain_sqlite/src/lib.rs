//! # Building blocks for persisted Holochain state
//!
//! ## Backend: LMDB
//!
//! Persistence is not generalized for different backends: it is targeted specifically for LMDB. In the future, if we have to change backends, or if we have to support something like IndexedDb, we will generalize the interface just enough to cover both. The interface provided by `rkv` is already somewhat generalized, with the abstract notions of Readers, Writers, and Stores.
//!
//! ## Buffered Stores
//!
//! The unit of persisted Holochain state is the [BufferedStore]. This interface groups three things together:
//!
//! - A reference to an LMDB database
//! - A reference to a read-only transaction (shared by other stores)
//! - A "scratch space", which is a HashMap into which write operations get staged (the buffer)
//!
//! The purpose of the scratch space is to prevent the need for opening a read-write transaction, of which there can be only one at a time. With the buffer of the scratch space, store references can live for a more leisurely length of time, accumulating changes, and then the buffer can be flushed all at once in a short-lived read-write transaction.
//!
//! Note that a BufferedStore includes a reference to a read-only transaction, which means that the store acts as a snapshot of the persisted data at the moment it was constructed. Changes to the underlying persistence will not be seen by this BufferedStore.
//!
//! See the [buffer] crate for implementations.
//!
//! ### Strong typing
//!
//! All BufferedStores are strongly typed. All keys and values must be de/serializable, and so de/serialization happens automatically when getting and putting items into stores. As a consequence, the source chain CAS is split into two separate DBs: one for Entries, and one for Headers.
//!
//! ## Workspaces
//!
//! The intention is that Holochain code never deals with individual data stores directly, individually. BufferedStores are always grouped into a Workspace, which is a collection of stores that's been put together for a specific purpose. A workspace may choose to provide open access to the underlying stores, or it may protect them behind a purpose-built interface.
//!
//! The stores in a Workspace are all provided a common read-only transaction, so their snapshots are all consistent with each other at the moment in time the workspace was constructed. The workspace provides its own interface for interacting with the stores. Once changes have been accumulated in the BufferedStores, the Workspace itself can be committed, which uses a fresh read-write transaction to flush the changes from each store and commit them to disk. Committing consumes the Workspace.
//!
//! Workspaces themselves are implemented in the `holochain` crate
//!
//! ## Building blocks
//!
//! The rkv crate provides a few abstractions for working with LMDB stores. The ones we use are:
//!
//! - SingleStore: a key-value store with arbitrary key and one value per key
//! - IntegerStore: a key-value store with integer key and one value per key
//! - MultiStore: a key-value store with arbitrary key and multiple values per key
//!
//! On top of these abstractions, the `crate` crate provides three buffered store abstractions to wrap each of the rkv store types, as well as a simple CAS abstraction:
//!
//! - [KvBuffer]: a SingleStore with a scratch space
//! - [KvIntBuffer]: an IntegerStore with a scratch space
//! - [KvvBuffer]: a MultiStore with a scratch space
//! - [CasBuffer]: a [KvBuffer] which enforces that keys must be the "address" of the values (content)
//!
//! The `holochain` crate composes these building blocks together to build more purpose-specific BufferedStore implementations
//!
//! See [this hackmd](https://holo.hackmd.io/@holochain/SkuVLpqEL) for a diagram explaining the relationships between these building blocks and the higher abstractions

#![allow(missing_docs)]

pub mod buffer;
pub mod db;
pub mod error;
pub mod exports;
pub mod fatal;
pub mod key;
pub mod prelude;
pub mod table;
pub mod transaction;

#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;
