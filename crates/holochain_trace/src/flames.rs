use tracing_core::field::Field;
use tracing_subscriber::field::Visit;

use chrono::SecondsFormat;
use std::path::PathBuf;

pub(crate) struct EventFieldFlameVisitor {
    pub samples: usize,
    name: &'static str,
}

impl EventFieldFlameVisitor {
    pub(crate) fn flame() -> Self {
        EventFieldFlameVisitor {
            samples: 0,
            name: "time.busy",
        }
    }
    pub(crate) fn ice() -> Self {
        EventFieldFlameVisitor {
            samples: 0,
            name: "time.idle",
        }
    }
}

impl Visit for EventFieldFlameVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == self.name {
            parse_time(&mut self.samples, value);
        }
    }
}

pub(crate) struct FlameTimed {
    path: PathBuf,
}

impl FlameTimed {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for FlameTimed {
    fn drop(&mut self) {
        save_flamegraph(self.path.clone());
    }
}

pub(crate) fn toml_path() -> Option<PathBuf> {
    let path = std::env::var_os("CARGO_MANIFEST_DIR").or_else(|| {
        println!("failed to get cargo manifest dir for flames");
        None
    })?;
    Some(PathBuf::from(path))
}

fn save_flamegraph(path: PathBuf) -> Option<()> {
    println!("path {:?}", path);
    let now = chrono::Local::now().to_rfc3339_opts(SecondsFormat::Secs, true);
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

fn parse_time(samples: &mut usize, value: &dyn std::fmt::Debug) {
    let v = format!("{:?}", value);
    if v.ends_with("ns") {
        if let Ok(v) = v.trim_end_matches("ns").parse::<f64>() {
            *samples = v as usize;
        }
    } else if v.ends_with("µs") {
        if let Ok(v) = v.trim_end_matches("µs").parse::<f64>() {
            *samples = (v * 1000.0) as usize;
        }
    } else if v.ends_with("ms") {
        if let Ok(v) = v.trim_end_matches("ms").parse::<f64>() {
            *samples = (v * 1000000.0) as usize;
        }
    } else if v.ends_with('s') {
        if let Ok(v) = v.trim_end_matches('s').parse::<f64>() {
            *samples = (v * 1000000000.0) as usize;
        }
    }
}
