use holochain::conductor::compat::load_conductor_from_legacy_config;
use holochain::conductor::config::ConductorConfig;
use holochain::conductor::error::ConductorError;
use holochain::conductor::interactive;
use holochain::conductor::paths::ConfigFilePath;
use holochain::conductor::Conductor;
use holochain::conductor::ConductorHandle;
use holochain_types::observability;
use holochain_types::observability::Output;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;
use tracing::*;

const ERROR_CODE: i32 = 42;
const MAGIC_CONDUCTOR_READY_STRING: &str = "Conductor ready.";

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
        long,
        help = "Path to a YAML file containing conductor configuration"
    )]
    config_path: Option<PathBuf>,

    #[structopt(
        long,
        help = "For backwards compatibility with Tryorama only: Path to a TOML file containing legacy conductor configuration"
    )]
    legacy_tryorama_config_path: Option<PathBuf>,

    #[structopt(
        short = "i",
        long,
        help = "Receive helpful prompts to create missing files and directories,
    useful when running a conductor for the first time"
    )]
    interactive: bool,
}

fn main() {
    holochain::conductor::tokio_runtime()
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

    let conductor = if let Some(legacy_config_path) = opt.legacy_tryorama_config_path {
        conductor_handle_from_legacy_config_path(&legacy_config_path).await
    } else {
        conductor_handle_from_config_path(opt.config_path.clone(), opt.interactive).await
    };

    info!("Conductor successfully initialized.");

    // This println has special meaning. Other processes can detect it and know
    // that the conductor has been initialized, in particular that the admin
    // interfaces are running, and can be connected to.
    println!("{}", MAGIC_CONDUCTOR_READY_STRING);

    // Await on the main JoinHandle, keeping the process alive until all
    // Conductor activity has ceased
    conductor
        .take_shutdown_handle()
        .await
        .expect("The shutdown handle has already been taken.")
        .await
        .map_err(|e| {
            error!(error = &e as &dyn Error, "Failed to join the main task");
            e
        })
        .expect("Error while joining threads during shutdown");

    // TODO: on SIGINT/SIGKILL, kill the conductor:
    // conductor.kill().await
}

async fn conductor_handle_from_legacy_config_path(legacy_config_path: &Path) -> ConductorHandle {
    let toml =
        fs::read_to_string(legacy_config_path).expect("Couldn't read legacy config from file");
    let legacy_config = toml::from_str(&toml).expect("Couldn't deserialize legacy config");
    load_conductor_from_legacy_config(legacy_config, Conductor::builder())
        .await
        .expect("Couldn't initialize conductor from legacy config")
}

async fn conductor_handle_from_config_path(
    config_path: Option<PathBuf>,
    interactive: bool,
) -> ConductorHandle {
    let config_path_default = config_path.is_none();
    let config_path: ConfigFilePath = config_path.map(Into::into).unwrap_or_default();
    debug!("config_path: {}", config_path);

    let config: ConductorConfig = if interactive {
        // Load config, offer to create default config if missing
        interactive::load_config_or_prompt_for_default(config_path)
            .expect("Could not load conductor config")
            .unwrap_or_else(|| {
                println!("Cannot continue without configuration");
                std::process::exit(ERROR_CODE);
            })
    } else {
        load_config(&config_path, config_path_default)
    };

    // Check if LMDB env dir is present
    // In interactive mode give the user a chance to create it, otherwise create it automatically
    let env_path = PathBuf::from(config.environment_path.clone());
    if !env_path.is_dir() {
        let result = if interactive {
            interactive::prompt_for_environment_dir(&env_path)
        } else {
            std::fs::create_dir_all(&env_path)
        };
        match result {
            Ok(()) => println!("Created LMDB environment at {}.", env_path.display()),
            Err(e) => {
                println!("Couldn't create LMDB environment: {}", e);
                std::process::exit(ERROR_CODE);
            }
        }
    }

    // Initialize the Conductor
    Conductor::builder()
        .config(config)
        .build()
        .await
        .expect("Could not initialize Conductor from configuration")
}

/// Load config, throw friendly error on failure
fn load_config(config_path: &ConfigFilePath, config_path_default: bool) -> ConductorConfig {
    match ConductorConfig::load_yaml(config_path.as_ref()) {
        Err(ConductorError::ConfigMissing(_)) => {
            display_friendly_missing_config_message(config_path, config_path_default);
            std::process::exit(ERROR_CODE);
        }
        Err(ConductorError::SerializationError(err)) => {
            display_friendly_malformed_config_message(config_path, err);
            std::process::exit(ERROR_CODE);
        }
        result => result.expect("Could not load conductor config"),
    }
}

fn display_friendly_missing_config_message(
    config_path: &ConfigFilePath,
    config_path_default: bool,
) {
    if config_path_default {
        println!(
            "
Error: The conductor is set up to load its configuration from the default path:

    {path}

but this file doesn't exist. If you meant to specify a path, run this command
again with the -c option. Otherwise, please either create a YAML config file at
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

but this file doesn't exist. Please either create a YAML config file at this
path yourself, or rerun the command with the '-i' flag, which will help you
automatically create a default config file.
        ",
            path = config_path,
        );
    }
}

fn display_friendly_malformed_config_message(
    config_path: &ConfigFilePath,
    error: serde_yaml::Error,
) {
    println!(
        "
The specified config file ({})
could not be parsed, because it is not valid YAML. Please check and fix the
file, or delete the file and run the conductor again with the -i flag to create
a valid default configuration. Details:

    {}

    ",
        config_path, error
    )
}
