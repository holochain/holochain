use hdk3::prelude::*;
// use hdk3::prelude::tracing_subscriber::FmtSubscriber;
// use hdk3::prelude::tracing_subscriber::fmt::time::FormatTime;

// use tracing_core::Subscriber;
// use tracing_subscriber::prelude::*;
// use tracing::Collect;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::fmt::Write;

#[derive(Default)]
struct WasmSubscriber {
    ids: AtomicUsize,
}

pub struct StringVisitor<'a> {
    fields: &'a mut String,
    message: &'a mut String,
}

impl<'a> tracing_core::field::Visit for StringVisitor<'a> {
    fn record_debug(&mut self, field: &tracing_core::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            write!(
                self.message,
                "{:?}",
                value
            ).unwrap();
        }
        else {
            write!(
                self.fields, 
                "{} = {:?}; ", 
                field.name(), 
                value
            ).unwrap();
        }
    }
}


impl tracing_core::Subscriber for WasmSubscriber {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }
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
        let mut visitor = StringVisitor{
            message: &mut String::new(),
            fields: &mut String::new(),
        };
        event.record(&mut visitor);
        host_call::<crate::prelude::TraceMsg, ()>(
            __trace,
            hdk3::prelude::TraceMsg {
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
        ).ok();
    }
    fn enter(&self, _span: &tracing::Id) {
        // unimplemented
    }
    fn exit(&self, _span: &tracing::Id) {
        // unimplemented
    }
}

#[hdk_extern]
fn debug(_: ()) -> ExternResult<()> {
    hdk3::trace!("HDK3 trace works!");
    hdk3::debug!("HDK3 debug works!");
    hdk3::info!("HDK3 info works!");
    hdk3::warn!("HDK3 warn works!");
    hdk3::error!("HDK3 error works!");

    tracing::subscriber::with_default(WasmSubscriber::default(), || {
        tracing::trace!("tracing {}", "works!");
        tracing::debug!("debug works");
        tracing::info!("info works");
        tracing::warn!("warn works");
        tracing::error!("error works");
        tracing::debug!(foo = "fields", bar = "work", "too")
    });

    Ok(())
}
