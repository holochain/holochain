//! Allows for adding serialized facts to logs, to be read out later

use std::sync::Arc;

use tracing_core::Subscriber;
use tracing_subscriber::{filter::filter_fn, registry::LookupSpan, Layer};

use crate::FactTraits;

pub trait FactLogTraits: FactTraits + serde::Serialize + serde::de::DeserializeOwned {}
impl<T> FactLogTraits for T where T: FactTraits + serde::Serialize + serde::de::DeserializeOwned {}

/// Add a JSON-serialized Fact to the tracing output at the Info level
#[macro_export]
macro_rules! trace {
    ($fact:expr) => {
        // The tracing level doesn't matter
        tracing::info!(
            aitia = "json",
            "<AITIA>{}</AITIA>",
            $crate::logging::LogLine::encode($fact)
        );
    };
}

/// Adds encode/decode functionality to a Fact so it can be logged
pub trait LogLine: FactLogTraits {
    /// Encode as string
    fn encode(&self) -> String;
    /// Decode from string
    fn decode(s: &str) -> Self;
}

/// A JSON-encoded fact
pub trait FactLogJson: LogLine {}

impl<J: FactLogJson> LogLine for J {
    fn encode(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    fn decode(s: &str) -> Self {
        serde_json::from_str(s).unwrap()
    }
}

pub trait Log: Default {
    type Fact: LogLine;
    fn parse(line: &str) -> Option<Self::Fact>;
    fn apply(&mut self, fact: Self::Fact);
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

#[derive(derive_more::Deref)]
pub struct AitiaWriter<L: Log>(Arc<std::sync::Mutex<L>>);

impl<L: Log> Clone for AitiaWriter<L> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<L: Log> Default for AitiaWriter<L> {
    fn default() -> Self {
        Self(Arc::new(std::sync::Mutex::new(L::default())))
    }
}

impl<L: Log> std::io::Write for AitiaWriter<L> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut g = self.0.lock().unwrap();
        let line = String::from_utf8_lossy(buf);
        let step = L::parse(&line).unwrap();
        g.apply(step);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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
