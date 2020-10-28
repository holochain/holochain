//! Utilities to make kitsune testing a little more sane.

use crate::{
    types::{actor::*, agent_store::*, event::*},
    *,
};
use futures::future::FutureExt;
use ghost_actor::dependencies::tracing;
use std::{collections::HashMap, sync::Arc};
use tokio::stream::StreamExt;

/// initialize tracing
pub fn init_tracing() {
    let _ = ghost_actor::dependencies::tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    );
}

/// Utility trait for test values
pub trait TestVal: Sized {
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
fn rand36<F: From<Vec<u8>>>() -> Arc<F> {
    use rand::Rng;
    let mut out = vec![0; 36];
    rand::thread_rng().fill(&mut out[..]);
    Arc::new(F::from(out))
}

// setup randomized TestVal::test_val() impls for kitsune data items
test_val! {
    Arc<KitsuneSpace> => { rand36() },
    Arc<KitsuneAgent> => { rand36() },
    Arc<KitsuneBasis> => { rand36() },
    Arc<KitsuneOpHash> => { rand36() },
}

/// a small debug representation of another type
#[derive(Clone)]
pub struct Slug(String);

impl std::fmt::Debug for Slug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

macro_rules! q_slug_from {
    ($($t:ty => |$i:ident| $c:block,)*) => {$(
        impl From<$t> for Slug {
            fn from(f: $t) -> Self {
                Slug::from(&f)
            }
        }

        impl From<&$t> for Slug {
            fn from(f: &$t) -> Self {
                let $i = f;
                Self($c)
            }
        }
    )*};
}

q_slug_from! {
    Arc<KitsuneSpace> => |s| {
        let f = format!("{:?}", s);
        format!("s{}", &f[13..25])
    },
    Arc<KitsuneAgent> => |s| {
        let f = format!("{:?}", s);
        format!("a{}", &f[13..25])
    },
}

mod harness_event;
use harness_event::*;

mod harness_agent;
pub(crate) use harness_agent::*;

mod harness_actor;
pub use harness_actor::*;
