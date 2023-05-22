use super::flames::*;
use tracing::{Event, Metadata, Subscriber};
use tracing_core::field::Field;
use tracing_serde::AsSerde;
use tracing_subscriber::{
    field::Visit,
    fmt::{format::Writer, FmtContext, FormatFields},
    registry::LookupSpan,
};

use serde_json::json;
use std::fmt::Write;

struct EventFieldVisitor {
    json: serde_json::Map<String, serde_json::Value>,
}

impl EventFieldVisitor {
    fn new() -> Self {
        let json = serde_json::Map::new();
        EventFieldVisitor { json }
    }
}

impl Visit for EventFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.json
            .insert(field.name().into(), json!(format!("{:?}", value)));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.json.insert(field.name().into(), json!(value));
    }
}

/// Formatting the events for json
pub(crate) struct FormatEvent;

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for FormatEvent
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let now = chrono::offset::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut parents = vec![];
        ctx.visit_spans::<(), _>(|span| {
            let meta = span.metadata();
            let name = meta.name();
            let file = meta.file();
            let line = meta.line();
            let module_path = meta.module_path();
            let level = meta.level();
            let target = meta.target();
            let id = span.id();
            let json = json!({"id": id.as_serde(), "name": name, "level": level.as_serde(), "target": target, "module_path": module_path, "file": file, "line": line});
            parents.push(json);
            Ok(())
        })
        .ok();
        let meta = event.metadata();
        let name = meta.name();
        let file = meta.file();
        let line = meta.line();
        let module_path = meta.module_path();
        let level = meta.level();
        let target = meta.target();
        let mut values = EventFieldVisitor::new();
        event.record(&mut values);
        let json = json!({"time": now, "name": name, "level": level.as_serde(), "target": target, "module_path": module_path, "file": file, "line": line, "fields": values.json, "spans": parents});
        writeln!(writer, "{}", json)
    }
}

/// Formatting the events for flame graphs
pub(crate) struct FormatEventFlame;

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for FormatEventFlame
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let mut values = EventFieldFlameVisitor::flame();
        event.record(&mut values);
        let mut stack = String::new();
        if values.samples > 0 {
            visit_parents(&mut stack, ctx);
            let event_data = event_data(event.metadata());
            writeln!(writer, "all; {} {} {}", stack, event_data, values.samples)
        } else {
            write!(writer, "")
        }
    }
}

/// Formatting the events for json
pub(crate) struct FormatEventIce;

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for FormatEventIce
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let mut values = EventFieldFlameVisitor::ice();
        event.record(&mut values);
        let mut stack = String::new();
        if values.samples > 0 {
            visit_parents(&mut stack, ctx);
            let event_data = event_data(event.metadata());
            writeln!(writer, "all; {} {} {}", stack, event_data, values.samples)
        } else {
            write!(writer, "")
        }
    }
}

fn event_data(meta: &Metadata) -> String {
    let mut event_data = String::new();
    if let Some(module) = meta.module_path() {
        write!(event_data, "{}:", module).ok();
    }
    if let Some(line) = meta.line() {
        write!(event_data, "{}", line).ok();
    }
    write!(event_data, ":{}", meta.name()).ok();
    event_data
}

fn visit_parents<S, N>(stack: &mut String, ctx: &FmtContext<'_, S, N>)
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    ctx.visit_spans::<(), _>(|span| {
        let meta = span.metadata();
        let name = meta.name();
        let module = meta.module_path();
        let line = meta.line();
        if let Some(module) = module {
            write!(stack, "{}:", module).ok();
        }
        if let Some(line) = line {
            write!(stack, "{}", line).ok();
        }
        write!(stack, ":{}", name).ok();
        *stack += "; ";
        Ok(())
    })
    .ok();
}
