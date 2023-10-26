//! Allows for adding serialized facts to logs, to be read out later

use tracing_core::Subscriber;
use tracing_subscriber::{filter::filter_fn, registry::LookupSpan, Layer};

use crate::Fact;

/// Add a JSON-serialized Fact to the tracing output at the Info level
#[macro_export]
macro_rules! trace {
    ($fact:expr) => {
        tracing::info!(aitia = "json", "{}", FactLog::encode($fact));
    };
}

/// Adds encode/decode functionality to a Fact so it can be logged
pub trait FactLog: Fact + serde::Serialize + serde::de::DeserializeOwned {
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

    use super::{layer, FactLog, FactLogJson};

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
        type Context = ();

        fn cause(&self, _ctx: &Self::Context) -> Option<crate::Cause<Self>> {
            todo!()
        }

        fn check(&self, _ctx: &Self::Context) -> bool {
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
