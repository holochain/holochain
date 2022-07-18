use holochain::conductor::config::ConductorConfig;
use holochain::conductor::interactive;
use holochain::conductor::manager::handle_shutdown;
use holochain::conductor::paths::ConfigFilePath;
use holochain::conductor::Conductor;
use holochain::conductor::ConductorHandle;
use holochain_conductor_api::conductor::ConductorConfigError;
use holochain_conductor_api::config::conductor::KeystoreConfig;
use holochain_util::tokio_helper;
use kitsune_p2p_types::dependencies::lair_keystore_api::LairResult;
use observability::Output;
#[cfg(unix)]
use sd_notify::{notify, NotifyState};
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

    /// Instead of the normal "interactive" method of passphrase
    /// retreival, read the passphrase from stdin. Be careful
    /// how you make use of this, as it could be less secure,
    /// for example, make sure it is not saved in your
    /// `~/.bash_history`.
    #[structopt(short = "p", long)]
    pub piped: bool,

    #[structopt(
        short = "i",
        long,
        help = "Receive helpful prompts to create missing files and directories,
    useful when running a conductor for the first time"
    )]
    interactive: bool,

    #[structopt(
        long,
        help = "Display version information such as git revision and HDK version"
    )]
    build_info: bool,
}

fn main() {
    // the async_main function should only end if our program is done
    tokio_helper::block_forever_on(async_main())
}

async fn async_main() {
    // Sets up a human-readable panic message with a request for bug reports
    //
    // See https://docs.rs/human-panic/1.0.3/human_panic/
    human_panic::setup_panic!();

    let opt = Opt::from_args();

    if opt.build_info {
        println!("{}", option_env!("BUILD_INFO").unwrap_or("{}"));
        return;
    }

    observability::init_fmt(opt.structured.clone()).expect("Failed to start contextual logging");
    debug!("observability initialized");

    kitsune_p2p_types::metrics::init_sys_info_poll();

    let conductor = conductor_handle_from_config_path(&opt).await;

    info!("Conductor successfully initialized.");

    // This println has special meaning. Other processes can detect it and know
    // that the conductor has been initialized, in particular that the admin
    // interfaces are running, and can be connected to.
    println!("{}", MAGIC_CONDUCTOR_READY_STRING);

    // Lets systemd units know that holochain is ready via sd_notify socket
    // Requires NotifyAccess=all and Type=notify attributes on holochain systemd unit
    // and NotifyAccess=all on dependant systemd unit
    #[cfg(unix)]
    let _ = notify(true, &[NotifyState::Ready]);

    // Await on the main JoinHandle, keeping the process alive until all
    // Conductor activity has ceased
    let result = conductor
        .take_shutdown_handle()
        .expect("The shutdown handle has already been taken.")
        .await;

    handle_shutdown(result);

    // TODO: on SIGINT/SIGKILL, kill the conductor:
    // conductor.kill().await
}

fn vec_to_locked(mut pass_tmp: Vec<u8>) -> LairResult<sodoken::BufRead> {
    match sodoken::BufWrite::new_mem_locked(pass_tmp.len()) {
        Err(e) => {
            pass_tmp.fill(0);
            Err(e)
        }
        Ok(p) => {
            {
                let mut lock = p.write_lock();
                lock.copy_from_slice(&pass_tmp);
                pass_tmp.fill(0);
            }
            Ok(p.to_read())
        }
    }
}

async fn read_interactive_passphrase(prompt: &str) -> LairResult<sodoken::BufRead> {
    let prompt = prompt.to_owned();
    let pass_tmp = tokio::task::spawn_blocking(move || {
        LairResult::Ok(
            rpassword::read_password_from_tty(Some(&prompt))
                .map_err(one_err::OneErr::new)?
                .into_bytes(),
        )
    })
    .await
    .map_err(one_err::OneErr::new)??;

    vec_to_locked(pass_tmp)
}

async fn read_piped_passphrase() -> LairResult<sodoken::BufRead> {
    use std::io::Read;
    Ok(tokio::task::spawn_blocking(move || {
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();
        let passphrase = <sodoken::BufWriteSized<512>>::new_mem_locked()?;
        let mut next_char = 0;
        loop {
            let mut lock = passphrase.write_lock();
            let done = match stdin.read_exact(&mut lock[next_char..next_char + 1]) {
                Ok(_) => {
                    if lock[next_char] == 10 {
                        true
                    } else {
                        next_char += 1;
                        false
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => true,
                Err(e) => return Err(e),
            };
            if done {
                if next_char == 0 {
                    return Ok(sodoken::BufWrite::new_no_lock(0).to_read());
                }
                if lock[next_char - 1] == 13 {
                    next_char -= 1;
                }
                let out = sodoken::BufWrite::new_mem_locked(next_char)?;
                {
                    let mut out_lock = out.write_lock();
                    out_lock.copy_from_slice(&lock[..next_char]);
                }
                return Ok(out.to_read());
            }
        }
    })
    .await
    .map_err(one_err::OneErr::new)??)
}

async fn conductor_handle_from_config_path(opt: &Opt) -> ConductorHandle {
    let config_path = opt.config_path.clone();
    let config_path_default = config_path.is_none();
    let config_path: ConfigFilePath = config_path.map(Into::into).unwrap_or_default();
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
        load_config(&config_path, config_path_default)
    };

    // read the passphrase to prepare for usage
    let passphrase = match &config.keystore {
        KeystoreConfig::DangerTestKeystoreLegacyDeprecated => None,
        KeystoreConfig::LairServerLegacyDeprecated { .. } => None,
        _ => {
            if opt.piped {
                Some(read_piped_passphrase().await.unwrap())
            } else {
                Some(
                    read_interactive_passphrase("\n# passphrase> ")
                        .await
                        .unwrap(),
                )
            }
        }
    };

    // Check if database is present
    // In interactive mode give the user a chance to create it, otherwise create it automatically
    let env_path = PathBuf::from(config.environment_path.clone());
    if !env_path.is_dir() {
        let result = if opt.interactive {
            interactive::prompt_for_database_dir(&env_path)
        } else {
            std::fs::create_dir_all(&env_path)
        };
        match result {
            Ok(()) => println!("Created database at {}.", env_path.display()),
            Err(e) => {
                println!("Couldn't create database: {}", e);
                std::process::exit(ERROR_CODE);
            }
        }
    }

    // Initialize the Conductor
    Conductor::builder()
        .config(config)
        .passphrase(passphrase)
        .build()
        .await
        .expect("Could not initialize Conductor from configuration")
}

/// Load config, throw friendly error on failure
fn load_config(config_path: &ConfigFilePath, config_path_default: bool) -> ConductorConfig {
    match ConductorConfig::load_yaml(config_path.as_ref()) {
        Err(ConductorConfigError::ConfigMissing(_)) => {
            display_friendly_missing_config_message(config_path, config_path_default);
            std::process::exit(ERROR_CODE);
        }
        Err(ConductorConfigError::SerializationError(err)) => {
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
