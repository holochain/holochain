use crate::db_explorer::state::ExplorerState;
use anyhow::anyhow;
use clap::{Arg, Command, Parser, Subcommand, ValueEnum};
use holochain::core::DnaHash;
use holochain_conductor_api::conductor::ConductorConfig;
use shlex;
use std::io::Write;
use std::path::{Path, PathBuf};

// configure /var/folders/8g/tvqzjrfj4d529fg72xp0g50h0000gp/T/bvxlPkFxdk2TU9VU2-qu7/conductor-config.yaml --admin-port 56152

#[derive(Parser)]
#[command(multicall = true)]
struct DbExplorerCli {
    #[command(subcommand)]
    command: DbExplorerCommand,
}

#[derive(Subcommand)]
enum DbExplorerCommand {
    /// Configure the conductor to use by providing its config file
    Configure {
        /// The root directory for the conductor
        #[arg(value_name = "DIR")]
        dir: PathBuf,

        /// Override the admin port found in the conductor config (e.g. when it is 0)
        #[arg(long, value_name = "PORT")]
        admin_port: Option<u16>,
    },

    /// List currently installed DNA hashes
    ListDna,

    /// List agents for a DNA
    ListAgents { dna_hash: String },

    /// Select a DNA (by hash) and database type to use
    Use {
        #[arg(value_enum)]
        kind: DbKind,
        hash: String,
    },

    /// Dump a source chain
    Dump { agent_public_key: String },

    /// Exit the explorer
    Quit,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum DbKind {
    Authored,
    Dnt,
}

pub async fn run_db_explorer() -> anyhow::Result<()> {
    let mut state = ExplorerState::new();

    loop {
        let line = readline()?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match respond(line, &mut state).await {
            Ok(quit) => {
                if quit {
                    break;
                }
            }
            Err(err) => {
                write!(std::io::stdout(), "{err}")?;
                std::io::stdout().flush()?;
            }
        }
    }

    Ok(())
}

async fn respond(line: &str, state: &mut ExplorerState) -> anyhow::Result<bool> {
    let args = shlex::split(line).ok_or(anyhow!("Invalid quoting"))?;
    match DbExplorerCli::try_parse_from(args)?.command {
        DbExplorerCommand::Configure { dir, admin_port } => {
            write!(std::io::stdout(), "Attempting to load {:?}\n", dir)?;
            state.reconfigure(PathBuf::from(dir), admin_port).await;
            println!("Done");
            std::io::stdout().flush()?;
        }
        DbExplorerCommand::ListDna => {
            state.list_dnas().await;
        }
        DbExplorerCommand::ListAgents { dna_hash } => {
            state.list_agents(dna_hash).await;
        }
        DbExplorerCommand::Use { kind, hash } => {
            let dna_hash = DnaHash::try_from(hash.as_str()).unwrap();
            state.use_db(kind, dna_hash).await;
        }
        DbExplorerCommand::Dump { agent_public_key } => {
            state.dump(agent_public_key).await;
        }
        DbExplorerCommand::Quit => {
            write!(std::io::stdout(), "Exiting ...")?;
            std::io::stdout().flush()?;
            return Ok(true);
        }
    }

    Ok(false)
}

fn readline() -> anyhow::Result<String> {
    write!(std::io::stdout(), "$ ")?;
    std::io::stdout().flush()?;
    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer)?;
    Ok(buffer)
}

mod state {
    use crate::db_explorer::DbKind;
    use holochain::conductor::space::Spaces;
    use holochain::core::DnaHash;
    use holochain::prelude::kitsune_p2p::dependencies::url2::url2;
    use holochain::prelude::{AgentPubKey, ChainQueryFilter, DbKindT};
    use holochain::test_utils::itertools::Itertools;
    use holochain_conductor_api::conductor::{ConductorConfig, KeystoreConfig};
    use holochain_conductor_api::{AdminRequest, AdminResponse};
    use holochain_websocket::{
        WebsocketConfig, WebsocketError, WebsocketReceiver, WebsocketResult, WebsocketSender,
    };
    use std::path::PathBuf;
    use std::sync::Arc;
    use walkdir::WalkDir;
    use holochain::prelude::kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::lair_keystore_api::dependencies::sodoken;
    use holochain_keystore::lair_keystore::{spawn_lair_keystore, spawn_lair_keystore_in_proc};
    use holochain_keystore::MetaLairClient;

    pub struct ExplorerState {
        work_dir: Option<PathBuf>,
        conductor_config: Option<ConductorConfig>,
        spaces: Option<Spaces>,
        admin_ws: Option<WebsocketSender>,
        current_db_kind: Option<DbKind>,
        current_dna_hash: Option<DnaHash>,
    }

    impl ExplorerState {
        pub fn new() -> Self {
            Self {
                work_dir: None,
                conductor_config: None,
                spaces: None,
                admin_ws: None,
                current_db_kind: None,
                current_dna_hash: None,
            }
        }

        pub async fn reconfigure(&mut self, work_dir: PathBuf, admin_port: Option<u16>) {
            self.work_dir = Some(work_dir);
            self.conductor_config = self.try_load_conductor_config();

            self.refresh(admin_port).await;
        }

        pub async fn refresh(&mut self, admin_port: Option<u16>) {
            if let Some(cfg) = &self.conductor_config {
                self.spaces = Some(Spaces::new(cfg).unwrap());

                println!("Connecting to the conductor's admin interface");
                self.admin_ws = Some(
                    self.connect_admin_ws(admin_port.unwrap_or_else(|| {
                        cfg.admin_interfaces
                            .as_ref()
                            .map(|v| v.first().cloned())
                            .flatten()
                            .expect("No admin interface specified in conductor config")
                            .driver
                            .port()
                    }))
                    .await
                    .unwrap()
                    .0,
                );
            } else {
                eprintln!("No config, please run `configure` first");
            }
        }

        pub async fn list_dnas(&mut self) {
            let dnas = if let Some(ws) = &mut self.admin_ws {
                match ws.request(AdminRequest::ListDnas).await.unwrap() {
                    AdminResponse::DnasListed(dnas) => dnas,
                    _ => {
                        eprintln!("Unexpected response while listing DNAs");
                        return;
                    }
                }
            } else {
                eprintln!("No config, please run `configure` first");
                return;
            };

            if dnas.is_empty() {
                println!("No DNAs found")
            } else {
                println!("{:?}", dnas);
            }
        }

        pub async fn list_agents(&mut self, dna_hash: String) {
            let cell_ids = if let Some(ws) = &mut self.admin_ws {
                match ws.request(AdminRequest::ListCellIds).await.unwrap() {
                    AdminResponse::CellIdsListed(cell_ids) => cell_ids,
                    _ => {
                        eprintln!("Unexpected response while listing agents");
                        return;
                    }
                }
            } else {
                eprintln!("No config, please run `configure` first");
                return;
            };

            if cell_ids.is_empty() {
                println!("No agents found")
            } else {
                let dna_hash = DnaHash::try_from(dna_hash).unwrap();

                println!(
                    "{:?}",
                    cell_ids
                        .iter()
                        .filter(|c| c.dna_hash() == &dna_hash)
                        .map(|c| c.agent_pubkey().to_string())
                        .collect_vec()
                );
            }
        }

        pub async fn use_db(&mut self, kind: DbKind, dna_hash: DnaHash) {
            self.current_db_kind = Some(kind);
            self.current_dna_hash = Some(dna_hash);
        }

        pub async fn dump(&mut self, agent_public_key: String) {
            let agent_public_key = AgentPubKey::try_from(agent_public_key.as_str()).unwrap();

            let lair_client = match &self.conductor_config.as_ref().unwrap().keystore {
                KeystoreConfig::LairServer { connection_url } => {
                    let passphrase = sodoken::BufRead::from(&b"1234"[..]);
                    spawn_lair_keystore(connection_url.clone(), passphrase)
                        .await
                        .unwrap()
                }
                _ => {
                    unimplemented!("only know how to use a lair server")
                }
            };

            let space = self
                .spaces
                .as_ref()
                .unwrap()
                .get_or_create_space(self.current_dna_hash.as_ref().unwrap())
                .unwrap();

            let chain = space
                .source_chain(lair_client, agent_public_key)
                .await
                .unwrap();

            let all = chain.query(ChainQueryFilter::new()).await.unwrap();

            println!("all {:?}", all);
        }

        fn try_load_conductor_config(&self) -> Option<ConductorConfig> {
            if let Some(work_dir) = &self.work_dir {
                let files: Vec<_> = WalkDir::new(work_dir)
                    .max_depth(1)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|d| d.file_type().is_file())
                    .filter(|f| f.file_name().to_string_lossy().eq("conductor-config.yaml"))
                    .map(|f| f.into_path())
                    .collect();

                if files.len() == 1 {
                    ConductorConfig::load_yaml(files.first().unwrap()).ok()
                } else {
                    if files.len() > 1 {
                        eprintln!("Multiple possible conductor configs, specify a specific file rather than a directory");
                    } else {
                        eprintln!(
                            "No `conductor-config.yaml` found, please specify a specific file"
                        );
                    }
                    None
                }
            } else {
                None
            }
        }

        async fn connect_admin_ws(
            &self,
            port: u16,
        ) -> WebsocketResult<(WebsocketSender, WebsocketReceiver)> {
            if port == 0 {
                eprintln!("Cannot connect to port 0, please override the admin port with `configure <path> --admin-port <PORT>`");
                return Err(WebsocketError::Shutdown);
            }

            println!("Connecting to admin port {}", port);
            holochain_websocket::connect(
                url2!("ws://127.0.0.1:{}", port),
                Arc::new(WebsocketConfig::default()),
            )
            .await
        }

        // fn get_current_db(&self) -> Option<Box<>> {
        //     if let (Some(kind), Some(dna_hash)) = (&self.current_db_kind, &self.current_dna_hash) {
        //         let x: Box<dyn DbKindT> = if let Some(spaces) = &self.spaces {
        //             match kind {
        //                 DbKind::Authored => Box::new(spaces.authored_db(&dna_hash).unwrap()),
        //                 DbKind::Dnt => Box::new(spaces.dht_db(&dna_hash).unwrap()),
        //             }
        //         } else {
        //             eprintln!("No config, please run `configure` first");
        //             return;
        //         };
        //
        //         self.current_db = Some(x);
        //     }
        // }
    }
}
