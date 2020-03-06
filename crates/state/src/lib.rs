#![feature(backtrace)]

pub mod buffer;
pub mod db;
pub mod env;
pub mod error;

// NB: would be nice to put this under cfg(test), but then it's not visible from other crates,
// since cfg(test) only applies to the crate in which you run tests
pub mod test_utils;

mod reader;
pub use reader::{Readable, Reader};

// Some re-exports

pub type Writer<'env> = rkv::Writer<'env>;
pub type SingleStore = rkv::SingleStore;
pub type IntegerStore = rkv::IntegerStore<u32>;
pub type MultiStore = rkv::MultiStore;
