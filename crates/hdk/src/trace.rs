use crate::prelude::*;
use std::fmt::Write;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Default)]
pub struct WasmSubscriber {
    ids: AtomicUsize,
}

/// Very basic struct to hold strings that can be written to by the tracing crate and sent to the host when complete.
struct StringVisitor<'a> {
    fields: &'a mut String,
    message: &'a mut String,
}

impl<'a> tracing_core::field::Visit for StringVisitor<'a> {
    fn record_debug(&mut self, field: &tracing_core::Field, value: &dyn std::fmt::Debug) {
        // Special case the message field so that it doesn't appear in the key/value format.
        if field.name() == "message" {
            let did_write = write!(self.message, "{:?}", value);
            if did_write.is_err() {
                let _ = write!(self.message, "**failed to write message**");
            }
        // Everything else is a simple key/value debug output.
        } else {
            let did_write = write!(self.fields, "{} = {:?}; ", field.name(), value);
            if did_write.is_err() {
                let _ = write!(self.fields, "**failed to write {}**", field.name());
            }
        }
    }
}

/// By implementing WasmSubscriber we integrate the rust tracing crate with the __trace host_fn without inventing some DIY DSL
///
/// Currently supports all the event macros for tracing such as `trace!`, `info!`, `debug!`, `warn!`, `error!`.
///
/// Does NOT support spans, so attempting to `#[instrument]` a function or similar will panic the wasm.
impl tracing_core::Subscriber for WasmSubscriber {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }
    /// Not really implemented yet.
    /// Placeholder logic only.
    fn new_span(&self, _attributes: &tracing_core::span::Attributes<'_>) -> tracing::Id {
        let next = self.ids.fetch_add(1, Ordering::SeqCst) as u64;
        tracing::Id::from_u64(next)
    }
    fn record(&self, _span: &tracing::Id, _values: &tracing::span::Record<'_>) {
        // unimplemented
    }
    fn record_follows_from(&self, _span: &tracing::Id, _follows: &tracing::Id) {
        // unimplemented
    }
    fn event(&self, event: &tracing::Event<'_>) {
        // New strings for every event.
        let mut visitor = StringVisitor {
            message: &mut String::new(),
            fields: &mut String::new(),
        };

        // This is handled by the Visit implementation above.
        event.record(&mut visitor);

        // The event is pushed to the host to be picked up by the subscriber on that side.
        // The visitor is dropped.
        host_call::<TraceMsg, ()>(
            __trace,
            TraceMsg {
                level: event.metadata().level().into(),
                msg: format!(
                    "{}:{}:{} {}{}",
                    event.metadata().module_path().unwrap_or(""),
                    event.metadata().file().unwrap_or(""),
                    event.metadata().line().unwrap_or(0),
                    visitor.fields,
                    visitor.message
                ),
            },
        )
        .ok();
    }
    fn enter(&self, _span: &tracing::Id) {
        // unimplemented
    }
    fn exit(&self, _span: &tracing::Id) {
        // unimplemented
    }
}
