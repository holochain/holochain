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
use crate::CmdRunner;
use anyhow::Context;
use clap::Parser;
use ed25519_dalek::Signer;
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppAuthenticationRequest, AppRequest, AppResponse, CellInfo,
    IssueAppAuthenticationTokenPayload,
};
use holochain_types::prelude::{
    AgentPubKey, CapAccess, CapSecret, DnaHashB64, ExternIO, FunctionName,
    GrantZomeCallCapabilityPayload, GrantedFunctions, InstalledAppId, SerializedBytes,
    SerializedBytesError, Signature, Timestamp, ZomeCallCapGrant, ZomeCallUnsigned, ZomeName,
    CAP_SECRET_BYTES,
};
use holochain_types::websocket::AllowedOrigins;
use holochain_websocket::{connect, ConnectRequest, WebsocketConfig, WebsocketReceiver};
use serde::{Deserialize, Serialize};
use sodoken::BufRead;
use std::collections::HashSet;
use std::net::ToSocketAddrs;
use std::sync::Arc;

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

    let app_client = AppClient::try_new(admin_port, zome_call_auth.app_id.clone()).await?;
    let app_info = app_client.request(AppRequest::AppInfo).await?;
    let info = match app_info {
        AppResponse::AppInfo(Some(info)) => info,
        other => anyhow::bail!("Unexpected response while getting app info: {:?}", other),
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

    let mut client = CmdRunner::try_new(admin_port).await?;

    holochain_util::pw::pw_set_piped(zome_call_auth.piped);
    if !zome_call_auth.piped {
        msg!("Enter new passphrase to authorize zome calls: ");
    }
    let passphrase = holochain_util::pw::pw_get().context("Failed to get passphrase")?;

    let (auth, key) = generate_signing_credentials(passphrase).await?;

    let signing_agent_key = AgentPubKey::from_raw_32(key.verifying_key().as_bytes().to_vec());

    for cell_id in cell_ids {
        client
            .command(AdminRequest::GrantZomeCallCapability(Box::new(
                GrantZomeCallCapabilityPayload {
                    cell_id: cell_id.clone(),
                    cap_grant: ZomeCallCapGrant::new(
                        "sandbox".to_string(),
                        CapAccess::Assigned {
                            secret: auth.cap_secret,
                            assignees: vec![signing_agent_key.clone()].into_iter().collect(),
                        },
                        GrantedFunctions::All,
                    ),
                },
            )))
            .await?;

        msg!("Authorized zome calls for cell: {:?}", cell_id);
    }

    Ok(())
}

pub async fn zome_call(zome_call: ZomeCall, admin_port: Option<u16>) -> anyhow::Result<()> {
    let admin_port = admin_port_from_connect_args(zome_call.connect_args, admin_port).await?;

    let client = AppClient::try_new(admin_port, zome_call.app_id.clone())
        .await
        .context("Could not create app client")?;

    let app_info = client.request(AppRequest::AppInfo).await?;
    let info = match app_info {
        AppResponse::AppInfo(Some(info)) => info,
        other => anyhow::bail!("Unexpected response while getting app info: {:?}", other),
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

    let (nonce, expires_at) = holochain_nonce::fresh_nonce(Timestamp::now())
        .map_err(|e| anyhow::anyhow!("Failed to generate nonce: {:?}", e))?;

    let params = ZomeCallUnsigned {
        provenance: AgentPubKey::from_raw_32(key.verifying_key().as_bytes().to_vec()),
        cell_id: cell_ids.first().unwrap().clone(),
        zome_name: ZomeName::from(zome_call.zome_name.clone()),
        fn_name: FunctionName(zome_call.function.clone()),
        cap_secret: Some(auth.cap_secret),
        payload: ExternIO::encode(serde_json::from_slice::<serde_json::Value>(
            zome_call.payload.as_bytes(),
        )?)?,
        nonce,
        expires_at,
    };
    let data_to_sign = params.data_to_sign()?;
    let sig = key.try_sign(&data_to_sign)?;

    let response = client
        .request(AppRequest::CallZome(Box::new(
            holochain_conductor_api::ZomeCall {
                cell_id: params.cell_id,
                zome_name: params.zome_name,
                fn_name: params.fn_name,
                payload: params.payload,
                cap_secret: params.cap_secret,
                provenance: params.provenance,
                nonce: params.nonce,
                expires_at: params.expires_at,
                signature: Signature::try_from(sig.to_vec())?,
            },
        )))
        .await?;

    match response {
        AppResponse::ZomeCalled(response) => {
            let response: hc_serde_json::Value = response.decode()?;
            serde_json::to_writer(std::io::stdout(), &response)?;
            println!(); // Add newline
        }
        other => anyhow::bail!("Unexpected response while calling zome: {:?}", other),
    }

    Ok(())
}

async fn generate_signing_credentials(
    passphrase: BufRead,
) -> anyhow::Result<(Auth, ed25519_dalek::SigningKey)> {
    let auth = load_or_create_auth().await?;

    let salt =
        sodoken::BufReadSized::<{ sodoken::hash::argon2id::SALTBYTES }>::from(auth.salt.as_ref());

    let hash = <sodoken::BufWriteSized<32>>::new_no_lock();
    {
        let read_guard = passphrase.read_lock();
        sodoken::hash::argon2id::hash(
            hash.clone(),
            read_guard.to_vec(),
            salt,
            sodoken::hash::argon2id::OPSLIMIT_INTERACTIVE,
            sodoken::hash::argon2id::MEMLIMIT_INTERACTIVE,
        )
        .await?;
    }

    Ok((
        auth,
        ed25519_dalek::SigningKey::from_bytes(hash.try_unwrap().unwrap().as_ref().try_into()?),
    ))
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

struct AppClient {
    ws_send: holochain_websocket::WebsocketSender,
    _recv: WsPollRecv,
}

impl AppClient {
    async fn try_new(admin_port: u16, installed_app_id: InstalledAppId) -> anyhow::Result<Self> {
        let mut cmd_runner = CmdRunner::try_new(admin_port).await?;

        let admin_response = cmd_runner.command(AdminRequest::ListAppInterfaces).await?;

        let app_interfaces = match admin_response {
            AdminResponse::AppInterfacesListed(app_interfaces) => app_interfaces,
            other => anyhow::bail!(
                "Unexpected response while listing app interfaces: {:?}",
                other
            ),
        };

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
                let response = cmd_runner
                    .command(AdminRequest::AttachAppInterface {
                        port: None,
                        allowed_origins: AllowedOrigins::Origins(
                            vec!["sandbox".to_string()].into_iter().collect(),
                        ),
                        installed_app_id: None,
                    })
                    .await?;

                match response {
                    AdminResponse::AppInterfaceAttached { port } => port,
                    other => anyhow::bail!(
                        "Unexpected response while attaching app interface: {:?}",
                        other
                    ),
                }
            }
        };

        let (ws_send, ws_recv) = connect(
            Arc::new(WebsocketConfig::CLIENT_DEFAULT),
            ConnectRequest::new(
                format!("localhost:{port}")
                    .to_socket_addrs()
                    .unwrap()
                    .next()
                    .unwrap(),
            )
            .try_set_header("origin", "sandbox")
            .unwrap(),
        )
        .await?;

        let _recv = WsPollRecv::new::<AppResponse>(ws_recv);

        let response = cmd_runner
            .command(AdminRequest::IssueAppAuthenticationToken(
                IssueAppAuthenticationTokenPayload::for_installed_app_id(installed_app_id.clone()),
            ))
            .await
            .context("Could not issue app authentication token")?;

        let token = match response {
            AdminResponse::AppAuthenticationTokenIssued(issued) => issued.token,
            other => anyhow::bail!(
                "Unexpected response while issuing app authentication token: {:?}",
                other
            ),
        };

        ws_send
            .authenticate(AppAuthenticationRequest { token })
            .await
            .context("Failed to authenticate app interface connection")?;

        Ok(Self { ws_send, _recv })
    }

    async fn request(&self, request: AppRequest) -> anyhow::Result<AppResponse> {
        Ok(self.ws_send.request(request).await?)
    }
}

/// You do not need to do anything with this type. While it is held it will keep polling a websocket
/// receiver.
struct WsPollRecv(tokio::task::JoinHandle<()>);

impl Drop for WsPollRecv {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl WsPollRecv {
    fn new<D>(mut rx: WebsocketReceiver) -> Self
    where
        D: std::fmt::Debug,
        SerializedBytes: TryInto<D, Error = SerializedBytesError>,
    {
        Self(tokio::task::spawn(async move {
            while rx.recv::<D>().await.is_ok() {}
        }))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Auth {
    salt: Vec<u8>,
    cap_secret: CapSecret,
}

async fn create_auth() -> anyhow::Result<Auth> {
    let salt = <sodoken::BufWriteSized<{ sodoken::hash::argon2id::SALTBYTES }>>::new_no_lock();
    sodoken::random::bytes_buf(salt.clone()).await?;
    let salt = salt.to_read_sized();

    let cap_secret = <sodoken::BufWriteSized<CAP_SECRET_BYTES>>::new_no_lock();
    sodoken::random::bytes_buf(cap_secret.clone()).await?;
    let cap_secret = cap_secret.to_read_sized();

    let auth = Auth {
        salt: salt.read_lock().as_ref().to_vec(),
        cap_secret: CapSecret::try_from(cap_secret.read_lock().as_ref().to_vec())?,
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
