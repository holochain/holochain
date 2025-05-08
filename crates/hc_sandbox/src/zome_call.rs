//! ## Making zome calls with the `hc sandbox zome-call` command
//!
//! To get started, you need a running conductor with an app installed. This example uses a sandbox
//! conductor with a single app.
//!
//! ```shell
//! hc sandbox generate --run=0 ./my-app.happ --app-id my-app
//! ```
//!
//! Enter a passphrase for the conductor when prompted. This passphrase is used to protect the Lair
//! keystore and an encryption key for Holochain data is derived from it.
//!
//! Now you have a running conductor, switch to a new shell and `cd` to the same directory that
//! you started the sandbox in. You should find a `.hc` file in this directory.
//!
//! Next we need to authorize zome calls for the app. Do this by running the following command,
//! providing the installed app id of the app you want to authorize zome calls for.
//!
//! ```shell
//! hc sandbox zome-call-auth my-app
//! ```
//!
//! You will be prompted for another passphrase. This is NOT the same as the conductor passphrase
//! you were prompted for above. This passphrase is used to derive a signing key for zome calls.
//! For testing, it is your choice whether to use the same passphrase. However, it is important to
//! understand that this passphrase is used to create signing keys that are valid when used
//! remotely. Using a simple or easily guessable passphrase here could allow somebody to make zome
//! calls as you, over the internet. They would need to know more than the passphrase to do this,
//! but it is recommended to use a strong passphrase.
//!
//! After entering a passphrase you should see a message logged for each cell in your app that zome
//! calls have been authorized for it. You are now ready to make a zome call.
//!
//! To make a zome call, you need a DNA hash, zome name, function name, and a payload. You can
//! get the DNA hash for a cell by running the following command:
//!
//! ```shell
//! hc sandbox call list-apps
//! ```
//!
//! Look for your app, and then look for a DNA hash that looks something like
//! `DnaHash(uhC0kIlrhnyl83p3E7PGwhNA3qx6who2f1W873C1xFQI_3SxnrR-A)`. It's the inner part that you
//! need to provide, which is the base64 encoded DNA hash.
//!
//! Now let's make a zome call:
//!
//! ```shell
//! hc sandbox zome-call my-app uhC0kIlrhnyl83p3E7PGwhNA3qx6who2f1W873C1xFQI_3SxnrR-A my-zome my-function '{"my": "payload"}'
//! ```
//!
//! You will be prompted for your password again. This is used in combination with the `.hc_auth`
//! file to re-generate your signing keys and to sign the zome call.
//!
//! Notice that the payload is provided as JSON. This is deserialized into a general data structure
//! that can accept any valid JSON. It is then converted to msgpack which is what the conductor
//! expects. The reverse is done with the zome call response. The msgpack is decoded into a general
//! data structure and then serialized back to JSON for output. You should see the result of your
//! zome call printed in your shell.
//!
//! There is a special case for calling a cell's `init` function. This hook does not require a
//! signed payload because it always runs as the agent that installed the DNA. This means that you
//! can skip the `zome-call-auth` step if you just want to initialise a cell.
//!
//! An existing conductor can be used with the `--running` or `--force-admin-ports` flags to the sandbox.
//! The force admin ports flag has a higher priority than the `--running` flag. Otherwise, the `.hc` file
//! in the current directory is used to find the admin port.
//!
//! These commands can also be used for headless operation by piping the passphrase in with the
//! `--piped` flag. Note that the `--piped` flag is part of the `zome-call-auth` and `zome-call`
//! commands and is not the same as the `--piped` global flag for the sandbox that allows you to
//! pipe the conductor passphrase. These flags aren't used together because the zome call commands
//! do not need the conductor passphrase.

use crate::cmds::Existing;
use crate::ports::get_admin_ports;
use anyhow::Context;
use clap::Parser;
use holochain_client::{
    AdminWebsocket, AppWebsocket, CellId, ClientAgentSigner, DynAgentSigner, SigningCredentials,
    ZomeCallTarget,
};
use holochain_conductor_api::{CellInfo, IssueAppAuthenticationTokenPayload};
use holochain_types::prelude::{
    AgentPubKey, CapAccess, CapSecret, DnaHashB64, ExternIO, FunctionName,
    GrantZomeCallCapabilityPayload, GrantedFunctions, InstalledAppId, ZomeCallCapGrant, ZomeName,
    CAP_SECRET_BYTES,
};
use holochain_types::websocket::AllowedOrigins;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Debug, Parser)]
pub struct ConnectArgs {
    /// Ports to running conductor admin interfaces.
    #[arg(short, long, conflicts_with_all = &["indices"], value_delimiter = ',')]
    pub running: Option<u16>,

    /// Select from existing conductor sandboxes specified in `$(pwd)/.hc` by index.
    #[arg(short, long, conflicts_with_all = &["running"])]
    pub index: Option<u32>,
}

/// Create authentication credentials for making zome calls and deploy them to Holochain.
#[derive(Debug, Parser)]
pub struct ZomeCallAuth {
    #[command(flatten)]
    pub connect_args: ConnectArgs,

    /// Whether to pipe the passphrase from stdin.
    ///
    /// By default, the passphrase is read interactively from the user.
    #[arg(long)]
    pub piped: bool,

    /// The installed app id to authorize calls for.
    pub app_id: String,
}

/// Make a zome call to an app on a running conductor.
#[derive(Debug, Parser)]
pub struct ZomeCall {
    #[command(flatten)]
    pub connect_args: ConnectArgs,

    /// Whether to pipe the passphrase from stdin.
    ///
    /// By default, the passphrase is read interactively from the user.
    #[arg(long)]
    pub piped: bool,

    /// The installed app id to call a function for
    pub app_id: String,

    /// The DNA hash to call
    pub dna_hash: DnaHashB64,

    /// The zome to call
    pub zome_name: String,

    /// The zome function to call
    pub function: String,

    /// The zome call payload as JSON
    pub payload: String,
}

pub async fn zome_call_auth(
    zome_call_auth: ZomeCallAuth,
    admin_port: Option<u16>,
) -> anyhow::Result<()> {
    let admin_port = admin_port_from_connect_args(zome_call_auth.connect_args, admin_port).await?;

    let admin_client = AdminWebsocket::connect(format!("localhost:{admin_port}")).await?;
    let app_client = get_app_client(&admin_client, zome_call_auth.app_id.clone(), None).await?;
    let app_info = app_client.app_info().await?;
    let info = match app_info {
        Some(info) => info,
        _ => anyhow::bail!("No app info found for app id {}", zome_call_auth.app_id),
    };

    let cell_ids = info
        .cell_info
        .values()
        .flatten()
        .filter_map(|info| match info {
            CellInfo::Provisioned(info) => Some(info.cell_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let admin_client = AdminWebsocket::connect(format!("localhost:{admin_port}")).await?;

    holochain_util::pw::pw_set_piped(zome_call_auth.piped);
    if !zome_call_auth.piped {
        msg!("Enter new passphrase to authorize zome calls: ");
    }
    let passphrase = holochain_util::pw::pw_get().context("Failed to get passphrase")?;

    let (auth, key) = generate_signing_credentials(passphrase).await?;

    let signing_agent_key = AgentPubKey::from_raw_32(key.verifying_key().as_bytes().to_vec());

    for cell_id in cell_ids {
        admin_client
            .grant_zome_call_capability(GrantZomeCallCapabilityPayload {
                cell_id: cell_id.clone(),
                cap_grant: ZomeCallCapGrant::new(
                    "sandbox".to_string(),
                    CapAccess::Assigned {
                        secret: auth.cap_secret,
                        assignees: vec![signing_agent_key.clone()].into_iter().collect(),
                    },
                    GrantedFunctions::All,
                ),
            })
            .await?;
        msg!("Authorized zome calls for cell: {:?}", cell_id);
    }

    Ok(())
}

pub async fn zome_call(zome_call: ZomeCall, admin_port: Option<u16>) -> anyhow::Result<()> {
    let admin_port = admin_port_from_connect_args(zome_call.connect_args, admin_port).await?;

    let admin_client = AdminWebsocket::connect(format!("localhost:{admin_port}")).await?;

    let app_client = get_app_client(&admin_client, zome_call.app_id.clone(), None).await?;

    let app_info = app_client.app_info().await?;
    let info = match app_info {
        Some(info) => info,
        _ => anyhow::bail!("No app info found for app id {}", zome_call.app_id.clone()),
    };

    let cell_ids = info
        .cell_info
        .values()
        .flatten()
        .filter_map(|info| match info {
            CellInfo::Provisioned(info)
                if info.cell_id.dna_hash() == zome_call.dna_hash.as_ref() =>
            {
                Some(info.cell_id.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if cell_ids.is_empty() {
        anyhow::bail!(
            "No cell found for DNA hash [{:?}] in app {:?}",
            zome_call.dna_hash,
            info
        );
    }

    holochain_util::pw::pw_set_piped(zome_call.piped);
    if !zome_call.piped {
        msg!("Enter passphrase to authorize zome calls: ");
    }
    let passphrase = holochain_util::pw::pw_get().context("Failed to get passphrase")?;

    let (auth, key) = generate_signing_credentials(passphrase).await?;

    let credentials: Vec<(CellId, SigningCredentials)> = cell_ids
        .clone()
        .into_iter()
        .map(|cell_id| {
            (
                cell_id,
                SigningCredentials {
                    signing_agent_key: AgentPubKey::from_raw_32(
                        key.verifying_key().as_bytes().to_vec(),
                    ),
                    keypair: key.clone(),
                    cap_secret: auth.cap_secret,
                },
            )
        })
        .collect();

    let app_client =
        get_app_client(&admin_client, zome_call.app_id.clone(), Some(credentials)).await?;

    let response = app_client
        .call_zome(
            ZomeCallTarget::CellId(cell_ids.first().unwrap().clone()),
            ZomeName::from(zome_call.zome_name),
            FunctionName(zome_call.function),
            ExternIO::encode(serde_json::from_slice::<serde_json::Value>(
                zome_call.payload.as_bytes(),
            )?)?,
        )
        .await?;

    serde_json::to_writer(std::io::stdout(), &response.decode::<serde_json::Value>()?)?;
    println!(); // Add newline

    Ok(())
}

async fn generate_signing_credentials(
    passphrase: Arc<Mutex<sodoken::LockedArray>>,
) -> anyhow::Result<(Auth, ed25519_dalek::SigningKey)> {
    let auth = load_or_create_auth().await?;

    let mut salt = [0; sodoken::argon2::ARGON2_ID_SALTBYTES];
    salt.copy_from_slice(&auth.salt);

    let hash = tokio::task::spawn_blocking(move || -> std::io::Result<[u8; 32]> {
        let mut hash = [0; 32];

        sodoken::argon2::blocking_argon2id(
            &mut hash,
            &passphrase.lock().unwrap().lock(),
            &salt,
            sodoken::argon2::ARGON2_ID_OPSLIMIT_INTERACTIVE,
            sodoken::argon2::ARGON2_ID_MEMLIMIT_INTERACTIVE,
        )?;

        Ok(hash)
    })
    .await??;

    Ok((auth, ed25519_dalek::SigningKey::from_bytes(&hash)))
}

async fn admin_port_from_connect_args(
    connect_args: ConnectArgs,
    admin_port: Option<u16>,
) -> anyhow::Result<u16> {
    // Use overridden admin port if provided, otherwise if the running argument was provided, use
    // that, otherwise load the existing paths from the .hc file, filter by index and get the
    // admin ports. If nothing is configured, load the `.hc` file from the current directory.
    if let Some(admin_port) = admin_port {
        Ok(admin_port)
    } else if let Some(admin_port) = connect_args.running {
        Ok(admin_port)
    } else if let Some(index) = connect_args.index {
        let paths = Existing {
            existing_paths: vec![],
            all: false,
            last: false,
            indices: vec![index as usize],
        }
        .load()?;

        if let Some(admin_port) = get_admin_ports(paths).await?.first() {
            Ok(*admin_port)
        } else {
            anyhow::bail!("No admin port found")
        }
    } else {
        let paths = crate::save::load(std::env::current_dir()?)?;

        if let Some(admin_port) = get_admin_ports(paths).await?.first() {
            Ok(*admin_port)
        } else {
            anyhow::bail!("No admin port found")
        }
    }
}

async fn get_app_client(
    admin_client: &AdminWebsocket,
    installed_app_id: InstalledAppId,
    credentials: Option<Vec<(CellId, SigningCredentials)>>,
) -> anyhow::Result<AppWebsocket> {
    let app_interfaces = admin_client.list_app_interfaces().await?;

    let existing_port = app_interfaces
        .iter()
        .filter_map(|app_interface| {
            if app_interface.installed_app_id.is_some()
                && app_interface.installed_app_id.as_ref().unwrap() != &installed_app_id
            {
                return None;
            }

            if app_interface.allowed_origins.is_allowed("sandbox") {
                Some(app_interface.port)
            } else {
                None
            }
        })
        .next();

    let port = match existing_port {
        Some(port) => port,
        None => {
            admin_client
                .attach_app_interface(
                    0,
                    AllowedOrigins::Origins(vec!["sandbox".to_string()].into_iter().collect()),
                    None,
                )
                .await?
        }
    };

    let token = admin_client
        .issue_app_auth_token(IssueAppAuthenticationTokenPayload::for_installed_app_id(
            installed_app_id.clone(),
        ))
        .await?;

    let signer = ClientAgentSigner::new();

    match credentials {
        None => (),
        Some(c) => {
            for (cell_id, creds) in c {
                signer.add_credentials(cell_id, creds);
            }
        }
    }

    Ok(AppWebsocket::connect(
        format!("localhost:{}", port),
        token.token,
        DynAgentSigner::from(signer),
    )
    .await?)
}

#[derive(Debug, Serialize, Deserialize)]
struct Auth {
    salt: Vec<u8>,
    cap_secret: CapSecret,
}

async fn create_auth() -> anyhow::Result<Auth> {
    let mut salt = [0; sodoken::argon2::ARGON2_ID_SALTBYTES];
    sodoken::random::randombytes_buf(&mut salt)?;

    let mut cap_secret = [0; CAP_SECRET_BYTES];
    sodoken::random::randombytes_buf(&mut cap_secret)?;

    let auth = Auth {
        salt: salt.to_vec(),
        cap_secret: CapSecret::from(cap_secret),
    };
    std::fs::write(".hc_auth", serde_json::to_vec(&auth)?)
        .context("Failed to write .hc_auth file")?;

    Ok(auth)
}

async fn load_or_create_auth() -> anyhow::Result<Auth> {
    if let Ok(content) = std::fs::read(".hc_auth") {
        Ok(serde_json::from_slice(&content)?)
    } else {
        create_auth().await
    }
}
