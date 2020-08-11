//! # Structured Contextual Logging (or tracing)
//! ## Why
//! [Watch](https://www.youtube.com/watch?v=JjItsfqFIdo) or [Read](https://tokio.rs/blog/2019-08-tracing/)
//!
//! ## Usage
//! There are a couple of ways to use structured logging.
//! ### Console and filter
//! If you want to try and filter in on an issue it might be easiest to simply log to the console and filter on what you want.
//! Here's an example command:
//! ```bash
//! CUSTOM_FILTER='core[a{something="foo"}]=debug' holochain --structured Log
//! ```
//! Or a more simple version using the default `Log`:
//! ```bash
//! RUST_LOG=trace holochain
//! ```
//! #### Filtering
//! ```bash
//! CUSTOM_FILTER='core[a{something="foo"}]=debug'
//! ```
//! Here we are saying show me all the events that are:
//! - In the `core` module
//! - Inside a span called `a`
//! - The span `a` has to have a field called `something` that is equal to `foo`
//! - They are atleast debug level.
//!
//! Most of these options are optional.
//! They can be combined like:
//! ```bash
//! CUSTOM_FILTER='[{}]=error, [{something}]=debug'
//! ```
//! > The above means show me errors from anywhere but also any event or span with the field something that's atleast debug.
//!
//! [See here](https://docs.rs/tracing-subscriber/0.2.2/tracing_subscriber/filter/struct.EnvFilter.html) for more info.
//!
//! #### Json
//! Sometimes there's too much data and it's better to capture it to interact with using another tool later.
//! For this we can output everything as Json using the flag `--structured Json`.
//! Then you can pipe the output from stdout to you're file of choice.
//! Here's some sample output:
//! ```json
//! {"time":"2020-03-03T08:07:05.910Z","name":"event crates/sim2h/src/sim2h_im_state.rs:695","level":"INFO","target":"sim2h::sim2h_im_state","module_path":"sim2h::sim2h_im_state","file":"crates/sim2h/src/sim2h_im_stat
//! e.rs","line":695,"fields":{"space_hashes":"[]"},"spans":[{"id":[1099511627778],"name":"check_gossip","level":"INFO","target":"sim2h::sim2h_im_state","module_path":"sim2h::sim2h_im_state","file":"crates/sim2h/src/s
//! im2h_im_state.rs","line":690}]}
//! ```
//! Every log will include the above information expect for the spans which will only show up if there are parent spans in the context of the event.
//!
//! You can combine filter with Json as well.
//!
//! ##### Tools
//! Some useful tools for formatting and using the json data.
//! - [json2csv](https://www.npmjs.com/package/json2csv)
//! - [jq](https://stedolan.github.io/jq/)
//! - [tad](https://www.tadviewer.com/)
//!
//! A sample workflow:
//! ```bash
//! CUSTOM_FILTER='core[{}]=debug' holochain --structured Json > log.json
//! cat out.json | jq '. | {time: .time, name: .name, message: .fields.message, file: .file, line: .line, fields: .fields, spans: .spans}' | json2csv -o log.csv
//! tad log.csv
//! ```

use tracing::{Event, Subscriber};
use tracing_core::field::Field;
use tracing_serde::AsSerde;
use tracing_subscriber::{
    field::Visit,
    filter::EnvFilter,
    fmt::{format::FmtSpan, time::ChronoUtc, FmtContext, FormatFields},
    prelude::__tracing_subscriber_SubscriberExt,
    registry::LookupSpan,
    FmtSubscriber, Registry,
};

use chrono::SecondsFormat;
use serde_json::json;
use std::fmt::Write;
use std::{path::PathBuf, str::FromStr, sync::Once};
use tracing_flame::FlameLayer;

#[derive(Debug, Clone)]
/// Sets the kind of structed logging output you want
pub enum Output {
    /// More compact version of above
    Compact,
    /// Outputs everything as json
    Json,
    /// Json with timed spans
    JsonTimed,
    /// Regular logging (default)
    Log,
    /// Regular logging plus timed spans
    LogTimed,
    /// Creates a flamegraph from timed spans
    FlameTimed,
    /// Creates a flamegraph from timed spans using idle time
    IceTimed,
    /// No logging to console
    None,
}

/// ParseError is a String
pub type ParseError = String;

static INIT: Once = Once::new();

impl FromStr for Output {
    type Err = ParseError;
    fn from_str(day: &str) -> Result<Self, Self::Err> {
        match day {
            "Json" => Ok(Output::Json),
            "JsonTimed" => Ok(Output::JsonTimed),
            "IceTimed" => Ok(Output::IceTimed),
            "Log" => Ok(Output::Log),
            "LogTimed" => Ok(Output::LogTimed),
            "FlameTimed" => Ok(Output::FlameTimed),
            "Compact" => Ok(Output::Compact),
            "None" => Ok(Output::None),
            _ => Err("Could not parse log output type".into()),
        }
    }
}

struct EventFieldFlameVisitor {
    samples: usize,
}

impl EventFieldFlameVisitor {
    fn new() -> Self {
        EventFieldFlameVisitor { samples: 0 }
    }
}

impl Visit for EventFieldFlameVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "time.busy" {
            parse_time(&mut self.samples, value);
        }
    }
}

struct EventFieldIceVisitor {
    samples: usize,
}

impl EventFieldIceVisitor {
    fn new() -> Self {
        EventFieldIceVisitor { samples: 0 }
    }
}

impl Visit for EventFieldIceVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "time.idle" {
            parse_time(&mut self.samples, value);
        }
    }
}

fn parse_time(samples: &mut usize, value: &dyn std::fmt::Debug) {
    let v = format!("{:?}", value);
    if v.ends_with("ns") {
        if let Some(v) = v.trim_end_matches("ns").parse::<f64>().ok() {
            *samples = v as usize;
        }
    } else if v.ends_with("µs") {
        if let Some(v) = v.trim_end_matches("µs").parse::<f64>().ok() {
            *samples = (v * 1000.0) as usize;
        }
    } else if v.ends_with("ms") {
        if let Some(v) = v.trim_end_matches("ms").parse::<f64>().ok() {
            *samples = (v * 1000000.0) as usize;
        }
    } else if v.ends_with("s") {
        if let Some(v) = v.trim_end_matches("s").parse::<f64>().ok() {
            *samples = (v * 1000000000.0) as usize;
        }
    }
}

struct Flame {
    guard: Option<Box<dyn Drop>>,
}

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

// Formating the events for json
fn format_event_flame<S, N>(
    ctx: &FmtContext<'_, S, N>,
    writer: &mut dyn std::fmt::Write,
    event: &Event<'_>,
) -> std::fmt::Result
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    let mut values = EventFieldFlameVisitor::new();
    event.record(&mut values);
    if values.samples > 0 {
        let mut parents = Vec::new();
        ctx.visit_spans::<(), _>(|span| {
            let mut stack = String::new();
            let meta = span.metadata();
            let name = meta.name();
            let file = meta.file();
            let line = meta.line();
            stack += "; ";
            write!(&mut stack, "{}", name).ok();
            if let Some(file) = file {
                write!(&mut stack, ":{}", file).ok();
            }
            if let Some(line) = line {
                write!(&mut stack, ":{}", line).ok();
            }
            parents.push(stack);
            Ok(())
        })
        .ok();
        let flat_parents: String = parents.iter().map(|n| n.chars()).rev().flatten().collect();
        writeln!(writer, "all{} {}", flat_parents, values.samples)
    } else {
        write!(writer, "")
    }
}

// Formating the events for json
fn format_event_ice<S, N>(
    ctx: &FmtContext<'_, S, N>,
    writer: &mut dyn std::fmt::Write,
    event: &Event<'_>,
) -> std::fmt::Result
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    let mut values = EventFieldIceVisitor::new();
    event.record(&mut values);
    if values.samples > 0 {
        let mut parents = Vec::new();
        ctx.visit_spans::<(), _>(|span| {
            let mut stack = String::new();
            let meta = span.metadata();
            let name = meta.name();
            let file = meta.file();
            let line = meta.line();
            stack += "; ";
            write!(&mut stack, "{}", name).ok();
            if let Some(file) = file {
                write!(&mut stack, ":{}", file).ok();
            }
            if let Some(line) = line {
                write!(&mut stack, ":{}", line).ok();
            }
            parents.push(stack);
            Ok(())
        })
        .ok();
        let flat_parents: String = parents.iter().map(|n| n.chars()).rev().flatten().collect();
        writeln!(writer, "all{} {}", flat_parents, values.samples)
    } else {
        write!(writer, "")
    }
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

/// Same as test_run but with timed spans
pub fn test_run_timed() -> Result<(), errors::TracingError> {
    if let (None, None) = (
        std::env::var_os("RUST_LOG"),
        std::env::var_os("CUSTOM_FILTER"),
    ) {
        return Ok(());
    }
    init_fmt(Output::LogTimed)
}

/// Same as test_run_timed but saves as json
pub fn test_run_timed_json() -> Result<(), errors::TracingError> {
    if let (None, None) = (
        std::env::var_os("RUST_LOG"),
        std::env::var_os("CUSTOM_FILTER"),
    ) {
        return Ok(());
    }
    init_fmt(Output::JsonTimed)
}

/// Same as test_run_timed but saves as json
pub fn test_run_timed_flame() -> Result<(), errors::TracingError> {
    if let (None, None) = (
        std::env::var_os("RUST_LOG"),
        std::env::var_os("CUSTOM_FILTER"),
    ) {
        return Ok(());
    }
    init_fmt(Output::FlameTimed)
}

/// Same as test_run_timed but saves as json
pub fn test_run_timed_ice() -> Result<(), errors::TracingError> {
    if let (None, None) = (
        std::env::var_os("RUST_LOG"),
        std::env::var_os("CUSTOM_FILTER"),
    ) {
        return Ok(());
    }
    init_fmt(Output::IceTimed)
}

/// Generate a tracing flamegraph for a test
/// The `RUST_LOG` filter needs to be set to the
/// spans you are interested in.
/// The file will be outputted at the `CARGO_MANIFEST_DIR` as
/// `tracing_flame_{date}.svg`.
/// You probably want to build you test as release with:
/// `cargo test --release`.
/// To avoid building the whole test suite twice it is recommended to use
/// an integration test like:
/// `cargo test --test my_integration_test --release`.
pub fn flame_run() -> Result<Option<impl Drop>, errors::TracingError> {
    if let None = std::env::var_os("RUST_LOG") {
        return Ok(None);
    }
    let filter = EnvFilter::from_default_env();
    let path = Flame::generate_path().ok_or(errors::TracingError::TracingFlame)?;
    let (flame_layer, guard) = FlameLayer::with_file(path.join("flames.folded"))?;

    let subscriber = Registry::default().with(filter).with(flame_layer);

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(Some(Flame {
        guard: Some(Box::new(guard)),
    }))
}

impl Flame {
    fn generate_path() -> Option<PathBuf> {
        let path = std::env::var_os("CARGO_MANIFEST_DIR").or_else(|| {
            println!("failed to get cargo manifest dir for flames");
            None
        })?;
        Some(PathBuf::from(path))
    }

    /// Save the flamegraph
    /// We don't want this to fail our tests but we want to know if it doesn't work
    fn flamegraph(&self) -> Option<()> {
        let now = chrono::Local::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let path = Flame::generate_path()?;
        let inf = std::fs::File::open(path.join("flames.folded"))
            .ok()
            .or_else(|| {
                eprintln!("failed to create flames dir");
                None
            })?;
        let reader = std::io::BufReader::new(inf);

        let out = std::fs::File::create(path.join(format!("tracing_flame_{}.svg", now)))
            .ok()
            .or_else(|| {
                eprintln!("failed to create flames inferno");
                None
            })?;
        let writer = std::io::BufWriter::new(out);

        let mut opts = inferno::flamegraph::Options::default();
        inferno::flamegraph::from_reader(&mut opts, reader, writer).unwrap();
        Some(())
    }
}

impl Drop for Flame {
    fn drop(&mut self) {
        if let Some(g) = self.guard.take() {
            drop(g);
        }
        self.flamegraph();
    }
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
    let fm_flame: fn(
        ctx: &FmtContext<'_, _, _>,
        &mut dyn std::fmt::Write,
        &Event<'_>,
    ) -> std::fmt::Result = format_event_flame;
    let fm_ice: fn(
        ctx: &FmtContext<'_, _, _>,
        &mut dyn std::fmt::Write,
        &Event<'_>,
    ) -> std::fmt::Result = format_event_ice;

    let subscriber = FmtSubscriber::builder()
        .with_target(true)
        .with_writer(std::io::stderr);

    match output {
        Output::Json => {
            let subscriber = subscriber
                .with_env_filter(filter)
                .with_timer(ChronoUtc::rfc3339())
                .json()
                .event_format(fm);
            finish(subscriber.finish())
        }
        Output::JsonTimed => {
            let subscriber = subscriber
                .with_span_events(FmtSpan::CLOSE)
                .with_env_filter(filter)
                .with_timer(ChronoUtc::rfc3339())
                .json()
                .event_format(fm);
            finish(subscriber.finish())
        }
        Output::Log => finish(subscriber.with_env_filter(filter).finish()),
        Output::LogTimed => {
            let subscriber = subscriber.with_span_events(FmtSpan::CLOSE);
            finish(subscriber.with_env_filter(filter).finish())
        }
        Output::FlameTimed => {
            let subscriber = subscriber
                .with_span_events(FmtSpan::CLOSE)
                .with_env_filter(filter)
                .with_timer(ChronoUtc::rfc3339())
                .event_format(fm_flame);
            finish(subscriber.finish())
        }
        Output::IceTimed => {
            let subscriber = subscriber
                .with_span_events(FmtSpan::CLOSE)
                .with_env_filter(filter)
                .with_timer(ChronoUtc::rfc3339())
                .event_format(fm_ice);
            finish(subscriber.finish())
        }
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
    //! Error in the tracing/logging framework

    use thiserror::Error;

    /// Error in the tracing/logging framework
    #[allow(missing_docs)] // should be self-explanitory
    #[derive(Error, Debug)]
    pub enum TracingError {
        #[error(transparent)]
        SetGlobal(#[from] tracing::subscriber::SetGlobalDefaultError),
        #[error("Failed to setup tracing flame")]
        TracingFlame,
        #[error(transparent)]
        TracingFlameError(#[from] tracing_flame::Error),
    }
}
