// FIXME: uncomment this deny [TK-01128]
// #![deny(missing_docs)]

#[macro_use]
mod fatal;

extern crate strum;
#[macro_use]
extern crate strum_macros;

pub mod conductor;
pub mod core;
pub mod fixt;
pub mod perf;
pub mod test_utils;

use holochain_wasmer_host;
