use std::path::PathBuf;

use holochain_conductor_api::{AdminRequest, AdminResponse};
use holochain_websocket::WebsocketSender;
use ports::get_admin_api;

pub use create::create;
pub use create::create_default;
pub use ports::add_secondary_admin_port;
pub use ports::force_admin_port;
pub use run::run;

macro_rules! msg {
    ($($arg:tt)*) => ({
        use ansi_term::Color::*;
        print!("{} ", Blue.bold().paint("hc-admin:"));
        println!($($arg)*);
    })
}

pub mod app;
pub mod calls;
pub mod cmds;
mod config;
mod create;
mod ports;
mod run;
pub mod scripts;

pub struct CmdRunner {
    client: WebsocketSender,
}

impl CmdRunner {
    /// Create a new connection for calling admin interface commands.
    /// Panics if admin port fails to connect.
    pub async fn new(port: u16) -> Self {
        Self::try_new(port)
            .await
            .expect("Failed to create CmdRunner because admin port failed to connect")
    }

    /// Create a new connection for calling admin interface commands.
    pub async fn try_new(port: u16) -> std::io::Result<Self> {
        let client = get_admin_api(port).await?;
        Ok(Self { client })
    }

    pub async fn command(&mut self, cmd: AdminRequest) -> anyhow::Result<AdminResponse> {
        tracing::debug!(?cmd);
        let response: Result<AdminResponse, _> = self.client.request(cmd).await;
        tracing::debug!(?response);
        Ok(response?)
    }
}

impl Drop for CmdRunner {
    fn drop(&mut self) {
        let f = self.client.close(0, "closing connection".to_string());
        tokio::task::spawn(f);
    }
}

#[macro_export]
macro_rules! expect_variant {
    ($var:expr => $variant:path, $error_msg:expr) => {
        match $var {
            $variant(v) => v,
            _ => panic!(format!("{}: Expected {} but got {:?}", $error_msg, stringify!($variant), $var)),
        }
    };
    ($var:expr => $variant:path) => {
        expect_variant!($var => $variant, "")
    };
}

#[macro_export]
macro_rules! expect_match {
    ($var:expr => $variant:path, $error_msg:expr) => {
        match $var {
            $variant(v) => v,
            _ => anyhow::bail!("{}: Expected {} but got {:?}", $error_msg, stringify!($variant), $var),
        }
    };
    ($var:expr => $variant:path) => {
        expect_variant!($var => $variant, "")
    };
}

pub fn save(mut path: PathBuf, paths: Vec<PathBuf>) -> anyhow::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(&path)?;
    path.push(".hc");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)?;

    for path in paths {
        writeln!(file, "{}", path.display())?;
    }
    Ok(())
}

pub fn clean(mut path: PathBuf, setups: Vec<usize>) -> anyhow::Result<()> {
    let existing = load(path.clone())?;
    let to_remove: Vec<_> = if setups.is_empty() {
        existing.iter().collect()
    } else {
        setups.into_iter().filter_map(|i| existing.get(i)).collect()
    };
    for p in to_remove {
        if p.exists() && p.is_dir() {
            if let Err(e) = std::fs::remove_dir_all(p) {
                tracing::error!("Failed to remove {} because {:?}", p.display(), e);
            }
        }
    }
    path.push(".hc");
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn load(mut path: PathBuf) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    path.push(".hc");
    if path.exists() {
        let existing = std::fs::read_to_string(path)?;
        for setup in existing.lines() {
            let path = PathBuf::from(setup);
            let mut config_path = path.clone();
            config_path.push("conductor-config.yaml");
            if config_path.exists() {
                paths.push(path);
            } else {
                tracing::error!("Failed to load path {} from existing .hc", path.display());
            }
        }
    }
    Ok(paths)
}

pub fn list(verbose: usize) -> anyhow::Result<()> {
    let out = load(std::env::current_dir()?)?
        .into_iter()
        .enumerate()
        .try_fold(
            "\nSetups contained in `.hc`\n".to_string(),
            |out, (i, path)| {
                let r = match verbose {
                    0 => format!("{}{}: {}\n", out, i, path.display()),
                    _ => {
                        let config = config::read_config(path.clone())?;
                        format!(
                            "{}{}: {}\nConductor Config:\n{:?}\n",
                            out,
                            i,
                            path.display(),
                            config
                        )
                    }
                };
                anyhow::Result::<_, anyhow::Error>::Ok(r)
            },
        )?;
    msg!("{}", out);
    Ok(())
}
