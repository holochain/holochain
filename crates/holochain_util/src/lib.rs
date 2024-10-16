//! This crate is a collection of various utility functions that are used by the other crates in the holochain repository.

#[cfg(feature = "fs")]
pub mod ffs;

#[cfg(feature = "tokio")]
pub mod tokio_helper;

#[cfg(feature = "pw")]
pub mod pw;

#[cfg(feature = "time")]
pub mod time;

pub mod hex;

pub use ::colored;
