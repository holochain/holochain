use holochain_conductor_api::{AdminRequest, AdminResponse};
use holochain_websocket::WebsocketSender;
use ports::get_admin_api;

pub use app::install_app;
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

mod app;
mod config;
mod create;
mod ports;
mod run;

pub struct CmdRunner {
    client: WebsocketSender,
}

impl CmdRunner {
    pub async fn new(port: u16) -> Self {
        let client = get_admin_api(port).await;
        Self { client }
    }

    pub async fn command(&mut self, cmd: AdminRequest) -> anyhow::Result<AdminResponse> {
        tracing::debug!(?cmd);
        let response: Result<AdminResponse, _> = self.client.request(cmd).await;
        tracing::debug!(?response);
        Ok(response?)
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
