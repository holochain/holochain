//! Allows for adding serialized facts to logs, to be read out later

use std::sync::Arc;

use base64::Engine;
use tracing_core::Subscriber;
use tracing_subscriber::{
    fmt::{writer::MakeWriterExt, MakeWriter},
    registry::LookupSpan,
    Layer,
};

use crate::{Fact, FactTraits};

pub trait FactLogTraits: FactTraits + serde::Serialize + serde::de::DeserializeOwned {}
impl<T> FactLogTraits for T where T: FactTraits + serde::Serialize + serde::de::DeserializeOwned {}

pub struct Event<F: Fact> {
    pub fact: F,
    pub timestamp: chrono::DateTime<chrono::Local>,
}

/// Add a JSON-serialized Fact to the tracing output at the Info level
#[macro_export]
macro_rules! trace {
    ($fact:expr) => {
        // Note the tracing level doesn't matter when using the AitiaWriter, but it
        // of course affects whether this will be present in the normal logs

        // XXX: because the JSON representation is wonky, especially for hashes,
        //      we also redundantly print a normal debug for better log readability
        let fact = $fact;

        let level = std::env::var("AITIA_LOG").unwrap_or("trace".to_string());
        match level.as_str() {
            "trace" => tracing::trace!(
                aitia = "json",
                ?fact,
                "<AITIA>{}</AITIA>",
                $crate::logging::FactLog::encode(fact)
            ),
            "debug" => tracing::debug!(
                aitia = "json",
                ?fact,
                "<AITIA>{}</AITIA>",
                $crate::logging::FactLog::encode(fact)
            ),
            "info" => tracing::info!(
                aitia = "json",
                ?fact,
                "<AITIA>{}</AITIA>",
                $crate::logging::FactLog::encode(fact)
            ),
            "warn" => tracing::warn!(
                aitia = "json",
                ?fact,
                "<AITIA>{}</AITIA>",
                $crate::logging::FactLog::encode(fact)
            ),
            "error" => tracing::error!(
                aitia = "json",
                ?fact,
                "<AITIA>{}</AITIA>",
                $crate::logging::FactLog::encode(fact)
            ),
            level => unimplemented!("Invalid AITIA_LOG setting: {}", level),
        }
    };
}

/// Adds encode/decode functionality to a Fact so it can be logged
pub trait FactLog: FactLogTraits {
    /// Encode as string
    fn encode(&self) -> String;
    /// Decode from string
    fn decode(s: &str) -> Self;
}

// /// A ready-made implementation of FactLog, using JSON-encoded facts
// pub trait FactLogJson: FactLog {}

// impl<J: FactLogJson> FactLog for J {
//     fn encode(&self) -> String {
//         serde_json::to_string(self).unwrap()
//     }

//     fn decode(s: &str) -> Self {
//         serde_json::from_str(s).unwrap()
//     }
// }

/// A ready-made implementation of FactLog, using msgpack-encoded facts with base-64 string encoding
pub trait FactLogMsgpackB64: FactLog {}

impl<J: FactLogMsgpackB64> FactLog for J {
    fn encode(&self) -> String {
        let bytes = rmp_serde::to_vec(self).unwrap();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
    }

    fn decode(s: &str) -> Self {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .unwrap();
        rmp_serde::from_slice(&bytes).unwrap()
    }
}

pub trait Log: Default {
    type Event: FactLog;

    fn parse(line: &str) -> Option<Self::Event> {
        regex::Regex::new("<AITIA>(.*?)</AITIA>")
            .unwrap()
            .captures(line)
            .and_then(|m| m.get(1))
            .map(|m| Self::Event::decode(m.as_str()))
    }

    fn apply(&mut self, fact: Self::Event);
}

/// A layer which only records logs emitted from aitia::trace!
/// This can be used to build up log state during a test run, instead of needing to
/// parse an entire completed log file, since the log file is still being written while
/// the test is running.
pub fn tracing_layer<S: Subscriber + for<'a> LookupSpan<'a>>(
    mw: impl for<'w> MakeWriter<'w> + 'static,
) -> impl Layer<S> {
    let mw = mw.with_filter(|metadata| metadata.fields().field("aitia").is_some());
    tracing_subscriber::fmt::layer()
        .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
        .with_writer(mw)
        .with_level(false)
        .with_file(true)
        .with_line_number(true)
}

#[derive(derive_more::Deref)]
pub struct AitiaSubscriber<L: Log>(Arc<parking_lot::Mutex<L>>);

impl<L: Log> Clone for AitiaSubscriber<L> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<L: Log> Default for AitiaSubscriber<L> {
    fn default() -> Self {
        Self(Arc::new(parking_lot::Mutex::new(L::default())))
    }
}

impl<L: Log> std::io::Write for AitiaSubscriber<L> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut g = self.0.lock();
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
    use tracing_subscriber::{
        prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Registry,
    };

    use crate::{dep::DepResult, logging::AitiaSubscriber, Fact};

    use super::{tracing_layer, FactLogMsgpackB64};

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

        fn dep(&self, _ctx: &Self::Context) -> DepResult<Self> {
            todo!()
        }

        fn check(&self, _ctx: &Self::Context) -> bool {
            todo!()
        }
    }

    impl FactLogMsgpackB64 for TestFact {}

    #[derive(Default)]
    struct Log(Vec<TestFact>);

    impl super::Log for Log {
        type Event = TestFact;

        fn apply(&mut self, fact: Self::Event) {
            self.0.push(fact)
        }
    }

    #[test]
    fn sample_log() {
        let log = AitiaSubscriber::<Log>::default();
        let log2 = log.clone();
        Registry::default()
            .with(tracing_layer(move || log2.clone()))
            .init();

        let facts = vec![
            TestFact::A("hello".to_string()),
            TestFact::B(24),
            TestFact::A("bye!".to_string()),
        ];

        for fact in facts.iter() {
            crate::trace!(fact);
        }

        {
            let ctx = log.lock();
            assert_eq!(ctx.0, facts);
        }
    }
}
