mod app;
mod cli;
mod client;
mod components;
mod event;
mod tui;

use crate::app::App;
use crate::cli::Args;
use crate::client::AdminClient;
use crate::event::handle_events;
use crate::tui::Tui;
use anyhow::anyhow;
use clap::Parser;
use holochain_util::tokio_helper::block_on;
use ratatui::prelude::*;
use std::io::{self};
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    args.validate()?;

    let (admin_client, app_client) = if let Some(admin_url) = &args.admin_url {
        let connect_clients_result = block_on(
            async {
                let addr = if let url::Origin::Tuple(_, host, port) = admin_url.origin() {
                    match tokio::net::lookup_host((host.to_string(), port)).await {
                        Ok(mut addr_list) => addr_list.next(),
                        Err(err) => return Err(anyhow!(err)),
                    }
                } else {
                    None
                };

                let addr = match addr {
                    None => return Err(anyhow!(format!("Invalid admin_url: {admin_url}"))),
                    Some(addr) => addr,
                };

                let mut admin_client = AdminClient::connect(addr).await?;
                let app_client = admin_client.connect_app_client().await?;

                Ok((admin_client, app_client))
            },
            Duration::from_secs(10),
        );
        match connect_clients_result {
            Ok(Ok((admin_client, app_client))) => (Some(admin_client), Some(app_client)),
            Ok(Err(e)) => {
                return Err(e);
            }
            Err(_) => {
                return Err(anyhow!("Timed out while connecting to Holochain"));
            }
        }
    } else {
        (None, None)
    };

    let mut app = App::new(args, admin_client, app_client, 2);

    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;
    let mut tui = Tui::new(terminal);
    tui.init()?;

    while app.is_running() {
        tui.draw(&mut app)?;
        handle_events(&mut app)?;
    }

    tui.exit()?;
    Ok(())
}
