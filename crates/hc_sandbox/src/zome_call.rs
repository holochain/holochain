use crate::cmds::Existing;
use crate::ports::get_admin_ports;
use crate::CmdRunner;
use anyhow::Context;
use clap::Parser;
use ed25519_dalek::Signer;
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppAuthenticationRequest, AppRequest, AppResponse, CellInfo,
    IssueAppAuthenticationTokenPayload, ZomeCallParamsSigned,
};
use holochain_types::prelude::{
    AgentPubKey, CapAccess, CapSecret, DnaHashB64, ExternIO, FunctionName,
    GrantZomeCallCapabilityPayload, GrantedFunctions, InstalledAppId, SerializedBytes,
    SerializedBytesError, Signature, Timestamp, ZomeCallCapGrant, ZomeCallParams, ZomeName,
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
    println!("Enter new passphrase to authorize zome calls: ");
    let passphrase = holochain_util::pw::pw_get().context("Failed to get passphrase")?;

    let (auth, key) = generate_signing_credentials(passphrase)?;

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

        println!("Authorized zome calls for cell: {:?}", cell_id);
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
        println!("Enter passphrase to authorize zome calls: ");
    }
    let passphrase = holochain_util::pw::pw_get().context("Failed to get passphrase")?;

    let (auth, key) = generate_signing_credentials(passphrase)?;

    let (nonce, expires_at) = holochain_nonce::fresh_nonce(Timestamp::now())
        .map_err(|e| anyhow::anyhow!("Failed to generate nonce: {:?}", e))?;

    let params = ZomeCallParams {
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
    let (payload, hash) = params.serialize_and_hash()?;

    let sig = key.try_sign(&hash)?;

    let response = client
        .request(AppRequest::CallZome(Box::new(ZomeCallParamsSigned {
            bytes: ExternIO::from(payload),
            signature: Signature::try_from(sig.to_vec())?,
        })))
        .await?;

    match response {
        AppResponse::ZomeCalled(response) => {
            let response: serde_json::Value = response.decode()?;
            serde_json::to_writer(std::io::stdout(), &response)?;
            println!(); // Add newline
        }
        other => anyhow::bail!("Unexpected response while calling zome: {:?}", other),
    }

    Ok(())
}

fn generate_signing_credentials(
    passphrase: BufRead,
) -> anyhow::Result<(Auth, ed25519_dalek::SigningKey)> {
    if unsafe { libsodium_sys::sodium_init() } < 0 {
        anyhow::bail!("Failed to initialize libsodium");
    }

    let auth = load_or_create_auth()?;

    let mut out = [0u8; 32];
    if unsafe {
        let read_guard = passphrase.read_lock();
        libsodium_sys::crypto_pwhash(
            out.as_mut_ptr(),
            out.len() as u64,
            std::ffi::CString::new(read_guard.as_ref())?.into_raw(),
            read_guard.len() as u64,
            auth.salt.as_ptr(),
            libsodium_sys::crypto_pwhash_OPSLIMIT_INTERACTIVE as u64,
            libsodium_sys::crypto_pwhash_MEMLIMIT_INTERACTIVE as usize,
            libsodium_sys::crypto_pwhash_ALG_DEFAULT as i32,
        ) != 0
    } {
        anyhow::bail!("Failed to derive key from password");
    }

    Ok((auth, ed25519_dalek::SigningKey::from_bytes(&out)))
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

fn create_auth() -> anyhow::Result<Auth> {
    use rand::RngCore;

    let mut salt = [0u8; libsodium_sys::crypto_pwhash_scryptsalsa208sha256_SALTBYTES as usize];
    unsafe {
        libsodium_sys::randombytes_buf(salt.as_mut_ptr() as *mut std::ffi::c_void, salt.len());
    }

    let mut csprng = rand::rngs::OsRng;
    let mut cap_secret = [0; CAP_SECRET_BYTES];
    csprng.fill_bytes(&mut cap_secret);

    let auth = Auth {
        salt: salt.to_vec(),
        cap_secret: CapSecret::try_from(cap_secret.to_vec())?,
    };
    std::fs::write(".hc_auth", serde_json::to_vec(&auth)?)
        .context("Failed to write .hc_auth file")?;

    Ok(auth)
}

fn load_or_create_auth() -> anyhow::Result<Auth> {
    if let Ok(content) = std::fs::read(".hc_auth") {
        Ok(serde_json::from_slice(&content)?)
    } else {
        create_auth()
    }
}
