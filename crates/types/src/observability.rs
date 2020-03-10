use tracing::{Event, Subscriber};
use tracing_core::field::Field;
use tracing_serde::AsSerde;
use tracing_subscriber::{
    field::Visit,
    filter::EnvFilter,
    fmt::{time::ChronoUtc, FmtContext, FormatFields},
    registry::LookupSpan,
    FmtSubscriber,
};

use serde_json::json;
use std::{str::FromStr, sync::Once};

#[derive(Debug)]
/// Sets the kind of structed logging output you want
pub enum Output {
    /// Outputs everything as json
    Json,
    /// Regular logging (default)
    Log,
    /// More compact version of above
    Compact,
    /// No logging to console
    None,
}

pub type ParseError = String;

static INIT: Once = Once::new();

impl FromStr for Output {
    type Err = ParseError;
    fn from_str(day: &str) -> Result<Self, Self::Err> {
        match day {
            "Json" => Ok(Output::Json),
            "Log" => Ok(Output::Log),
            "Compact" => Ok(Output::Compact),
            "None" => Ok(Output::None),
            _ => Err("Could not parse log output type".into()),
        }
    }
}

pub struct EventFieldVisitor {
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

// Formating the events for json
fn format_event<S, N>(
    ctx: &FmtContext<'_, S, N>,
    writer: &mut dyn std::fmt::Write,
    event: &Event<'_>,
) -> std::fmt::Result
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
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

/// Run logging in a unit test
/// RUST_LOG or CUSTOM_FILTER must be set or
/// this is a no-op
pub fn test_run() -> Result<(), errors::TracingError> {
    if let (None, None) = (
        std::env::var_os("RUST_LOG"),
        std::env::var_os("CUSTOM_FILTER"),
    ) {
        return Ok(());
    }
    init_fmt(Output::Log)
}

/// This checks RUST_LOG for a filter but doesn't complain if there is none or it doesn't parse.
/// It then checks for CUSTOM_FILTER which if set will output an error if it doesn't parse.
pub fn init_fmt(output: Output) -> Result<(), errors::TracingError> {
    let mut filter = EnvFilter::from_default_env();
    if std::env::var("CUSTOM_FILTER").is_ok() {
        EnvFilter::try_from_env("CUSTOM_FILTER")
            .map_err(|e| eprintln!("Failed to parse CUSTOM_FILTER {:?}", e))
            .map(|f| {
                filter = f;
            })
            .ok();
    }
    let fm: fn(
        ctx: &FmtContext<'_, _, _>,
        &mut dyn std::fmt::Write,
        &Event<'_>,
    ) -> std::fmt::Result = format_event;

    let subscriber = FmtSubscriber::builder().with_target(true);

    match output {
        Output::Json => {
            let subscriber = subscriber
                .with_env_filter(filter)
                .with_timer(ChronoUtc::rfc3339())
                .json()
                .event_format(fm);
            finish(subscriber.finish())
        }
        Output::Log => finish(subscriber.with_env_filter(filter).finish()),
        Output::Compact => {
            let subscriber = subscriber.compact();
            finish(subscriber.with_env_filter(filter).finish())
        }
        Output::None => Ok(()),
    }
}

fn finish<S>(subscriber: S) -> Result<(), errors::TracingError>
where
    S: Subscriber + Send + Sync + for<'span> LookupSpan<'span>,
{
    let mut result = Ok(());
    INIT.call_once(|| {
        result = tracing::subscriber::set_global_default(subscriber).map_err(Into::into);
    });
    result
}

pub mod errors {
    use thiserror::Error;
    #[derive(Error, Debug)]
    pub enum TracingError {
        #[error(transparent)]
        SetGlobal(#[from] tracing::subscriber::SetGlobalDefaultError),
    }
}
