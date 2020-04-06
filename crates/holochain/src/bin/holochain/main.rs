use holochain_2020::conductor::{
    api::*, config::ConductorConfig, error::ConductorError, interactive, interface::channel::*,
    paths::ConfigFilePath, Conductor,
};
use std::{convert::TryInto, path::PathBuf, sync::Arc};
use structopt::StructOpt;
use sx_types::observability::{self, Output};
use tokio::sync::RwLock;
use tracing::*;

const ERROR_CODE: i32 = 42;

#[derive(Debug, StructOpt)]
#[structopt(name = "holochain", about = "The Holochain Conductor.")]
struct Opt {
    #[structopt(
        long,
        help = "Outputs structured json from logging:
    - None: No logging at all (fastest)
    - Log: Output logs to stdout with spans (human readable)
    - Compact: Same as Log but with less information
    - Json: Output logs as structured json (machine readable)
    ",
        default_value = "Log"
    )]
    structured: Output,

    #[structopt(
        short = "c",
        help = "Path to a TOML file containing conductor configuration"
    )]
    config_path: Option<PathBuf>,

    #[structopt(
        short = "i",
        long,
        help = "Receive helpful prompts to create missing files and directories,
    useful when running a conductor for the first time"
    )]
    interactive: bool,

    #[structopt(
        long = "example",
        help = "Run a very basic interface example, just to have something to do"
    )]
    run_interface_example: bool,
}

fn main() {
    tokio::runtime::Builder::new()
        // we use both IO and Time tokio utilities
        .enable_all()
        // we want to use multiple threads
        .threaded_scheduler()
        // we want to use thread count matching cpu count
        // (sometimes tokio by default only uses half cpu core threads)
        .core_threads(num_cpus::get())
        // give our threads a descriptive name (they'll be numbered too)
        .thread_name("holochain-tokio-thread")
        // build the runtime
        .build()
        // panic if we cannot (we cannot run without it)
        .expect("can build tokio runtime")
        // the async_main function should only end if our program is done
        .block_on(async_main())
}

async fn async_main() {
    // Sets up a human-readable panic message with a request for bug reports
    //
    // See https://docs.rs/human-panic/1.0.3/human_panic/
    human_panic::setup_panic!();

    let opt = Opt::from_args();
    observability::init_fmt(opt.structured).expect("Failed to start contextual logging");
    debug!("observability initialized");

    let config_path_default = opt.config_path.is_none();
    let config_path: ConfigFilePath = opt.config_path.map(Into::into).unwrap_or_default();
    debug!("config_path: {}", config_path);

    let config: ConductorConfig = if opt.interactive {
        // Load config, offer to create default config if missing
        interactive::load_config_or_prompt_for_default(config_path)
            .expect("Could not load conductor config")
            .unwrap_or_else(|| {
                println!("Cannot continue without configuration");
                std::process::exit(ERROR_CODE);
            })
    } else {
        // Load config, throw friendly error on failure
        match ConductorConfig::load_toml(config_path.as_ref()) {
            Err(ConductorError::ConfigMissing(_)) => {
                display_friendly_missing_config_message(config_path, config_path_default);
                std::process::exit(ERROR_CODE);
            }
            Err(ConductorError::DeserializationError(err)) => {
                display_friendly_malformed_config_message(config_path, err);
                std::process::exit(ERROR_CODE);
            }
            result => result.expect("Could not load conductor config"),
        }
    };

    // If interactive mode, give the user a chance to create LMDB env if missing
    let env_path = PathBuf::from(config.environment_path.clone());
    if opt.interactive && !env_path.is_dir() {
        match interactive::prompt_for_environment_dir(&env_path) {
            Ok(true) => println!("LMDB environment created."),
            Ok(false) => {
                println!("Cannot continue without LMDB environment set.");
                std::process::exit(ERROR_CODE);
            }
            result => {
                result.expect("Couldn't auto-create environment dir");
            }
        }
    }

    // Initialize the Conductor
    let conductor: Conductor = Conductor::build()
        .from_config(config)
        .await
        .expect("Could not initialize Conductor from configuration");

    let lock = Arc::new(RwLock::new(conductor));
    // Create an external API to hand off to any Interfaces
    let api = RealExternalConductorApi::new(lock);

    if opt.run_interface_example {
        interface_example(api).await;
    } else {
        // TODO: kick off actual conductor task here when we're ready for that
        println!("Conductor successfully initialized. Nothing else to do. Bye bye!");
    }
}

fn display_friendly_missing_config_message(config_path: ConfigFilePath, config_path_default: bool) {
    if config_path_default {
        println!(
            "
Error: The conductor is set up to load its configuration from the default path:

    {path}

but this file doesn't exist. If you meant to specify a path, run this command
again with the -c option. Otherwise, please either create a TOML config file at
this path yourself, or rerun the command with the '-i' flag, which will help you
automatically create a default config file.
        ",
            path = config_path,
        );
    } else {
        println!(
            "
Error: You asked to load configuration from the path:

    {path}

but this file doesn't exist. Please either create a TOML config file at this
path yourself, or rerun the command with the '-i' flag, which will help you
automatically create a default config file.
        ",
            path = config_path,
        );
    }
}

fn display_friendly_malformed_config_message(config_path: ConfigFilePath, error: toml::de::Error) {
    println!(
        "
The specified config file ({})
could not be parsed, because it is not valid TOML. Please check and fix the
file, or delete the file and run the conductor again with the -i flag to create
a valid default configuration. Details:

    {}

    ",
        config_path, error
    )
}

/// Simple example of what an [Interface] looks like in its most basic form,
/// and how to interact with it.
/// TODO: remove once we have real Interfaces
async fn interface_example(api: RealExternalConductorApi) {
    let (mut sender, join_handle) = create_demo_channel_interface(api);
    let driver_fut = async move {
        for _ in 0..50 as u32 {
            use futures::{future::FutureExt, sink::SinkExt};
            sender
                .send((
                    InterfaceMsgIncoming::AdminRequest(Box::new(AdminRequest::Start(
                        "cell-handle".into(),
                    )))
                    .try_into()
                    .unwrap(),
                    Box::new(|_| async move { Ok(()) }.boxed()),
                ))
                .await
                .unwrap();
        }
    };
    driver_fut.await;
    join_handle.await.unwrap();
}
