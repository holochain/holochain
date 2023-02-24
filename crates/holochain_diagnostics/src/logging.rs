use std::{fs::File, path::Path};

use tracing_subscriber::{EnvFilter, FmtSubscriber};

pub fn setup_logging(file: Option<(&Path, bool)>) {
    let filter = match std::env::var("RUST_LOG") {
        Ok(_) => EnvFilter::from_default_env(),
        Err(_) => {
            println!("No logging setup.");
            return;
        }
    };

    let subscriber = FmtSubscriber::builder()
        .with_target(true)
        .with_env_filter(filter);

    if let Some((path, truncate)) = file {
        let p = path.to_owned();
        let writer = std::sync::Mutex::new(if truncate {
            File::create(&p).expect("Couldn't open file for logging")
        } else {
            File::open(&p).expect("Couldn't open file for logging")
        });
        let s = subscriber.with_writer(writer).finish();
        tracing::subscriber::set_global_default(s).unwrap();
    } else {
        let s = subscriber.with_writer(std::io::stderr).finish();
        tracing::subscriber::set_global_default(s).unwrap();
    };
}
