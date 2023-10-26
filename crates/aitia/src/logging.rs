//! Allows for adding serialized facts to logs, to be read out later

use std::collections::HashSet;

use tracing_core::Subscriber;
use tracing_subscriber::{filter::filter_fn, registry::LookupSpan, Layer};

use crate::FactTraits;

pub trait FactLogTraits: FactTraits + serde::Serialize + serde::de::DeserializeOwned {}
impl<T> FactLogTraits for T where T: FactTraits + serde::Serialize + serde::de::DeserializeOwned {}

/// Add a JSON-serialized Fact to the tracing output at the Info level
#[macro_export]
macro_rules! trace {
    ($fact:expr) => {
        tracing::info!(
            aitia = "json",
            "<AITIA>{}</AITIA>",
            $crate::logging::FactLog::encode($fact)
        );
    };
}

/// Adds encode/decode functionality to a Fact so it can be logged
pub trait FactLog: FactLogTraits {
    /// Encode as string
    fn encode(&self) -> String;
    /// Decode from string
    fn decode(s: &str) -> Self;
}

/// A JSON-encoded fact
pub trait FactLogJson: FactLog {}

impl<J: FactLogJson> FactLog for J {
    fn encode(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    fn decode(s: &str) -> Self {
        serde_json::from_str(s).unwrap()
    }
}

pub trait Log<F: FactLog> {
    fn parse(line: &str) -> Option<F>;
    fn apply(self, fact: F) -> Self;
}

// pub struct LogAccumulator<F: FactLog> {
//     facts: HashSet<F>,
// }

/// A layer which only records logs emitted from aitia::trace!
pub fn layer<S: Subscriber + for<'a> LookupSpan<'a>>() -> impl Layer<S> {
    tracing_subscriber::fmt::layer()
        .with_test_writer()
        .with_level(false)
        .with_file(true)
        .with_line_number(true)
        .with_filter(filter_fn(|metadata| {
            metadata.fields().field("aitia").is_some()
        }))
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, Registry};

    use crate::Fact;

    use super::{layer, FactLogJson};

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Hash,
        derive_more::Display,
        serde::Serialize,
        serde::Deserialize,
    )]
    enum TestFact {
        A(String),
        B(u32),
    }

    impl Fact for TestFact {
        fn cause(&self, _ctx: &()) -> Option<crate::Cause<Self>> {
            todo!()
        }
    }

    impl FactLogJson for TestFact {}

    #[test]
    fn sample_log() {
        tracing::subscriber::set_global_default(Registry::default().with(layer::<_>())).unwrap();

        crate::trace!(&TestFact::A("hello".to_string()));
    }
}
