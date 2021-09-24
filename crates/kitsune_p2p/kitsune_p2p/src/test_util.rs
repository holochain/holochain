//! Utilities to make kitsune testing a little more sane.

#![allow(dead_code)]

use crate::types::actor::*;
use crate::types::agent_store::*;
use crate::types::event::*;
use crate::*;
use futures::future::FutureExt;
use ghost_actor::dependencies::tracing;
use std::collections::HashMap;
use std::sync::Arc;

/// Utility trait for test values
pub trait TestVal: Sized {
    /// Create the test val
    fn test_val() -> Self;
}

/// Boilerplate shortcut for implementing TestVal on an item
#[macro_export]
macro_rules! test_val  {
    ($($item:ty => $code:block,)*) => {$(
        impl TestVal for $item { fn test_val() -> Self { $code } }
    )*};
}

/// internal helper to generate randomized kitsune data items
fn rand36<F: KitsuneBinType>() -> Arc<F> {
    use rand::Rng;
    let mut out = vec![0; 36];
    rand::thread_rng().fill(&mut out[..]);
    Arc::new(F::new(out))
}

// setup randomized TestVal::test_val() impls for kitsune data items
test_val! {
    Arc<KitsuneSpace> => { rand36() },
    Arc<KitsuneAgent> => { rand36() },
    Arc<KitsuneBasis> => { rand36() },
    Arc<KitsuneOpHash> => { rand36() },
}

mod gossip_scenario;
pub use gossip_scenario::*;

mod harness_event;
pub(crate) use harness_event::*;

mod harness_agent;
pub(crate) use harness_agent::*;

mod harness_actor;
#[allow(unused_imports)]
pub(crate) use harness_actor::*;

// NB: this is a copy of `KdMetricStore` from `kitsune_p2p_direct`, which
//   is downstream of this crate.
//   Since we plan to delete much of these tests, I opted to keep that type
//   downstream, and just copy it upstream here for now.
mod metric_store;
pub use metric_store::*;

pub mod scenario_def;
pub(crate) mod scenario_def_local;
