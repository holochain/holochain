use std::path::PathBuf;

use holochain_cli_sandbox as hc_sandbox;
use holochain_cli_sandbox::run;
use holochain_client::AdminWebsocket;
use holochain_trace::Output;
use holochain_types::prelude::AppBundleSource;
use holochain_types::prelude::InstallAppPayload;

use clap::Parser;

#[derive(Debug, Parser)]
struct Input {
    #[arg(short = 'H', long, default_value = "holochain")]
    holochain_path: PathBuf,
    happ: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get and parse any input.
    let input = Input::parse();
    let happ = hc_sandbox::bundles::parse_happ(input.happ)?;

    // Choose an app id and properties.
    let app_id = "my-cool-app".to_string();

    for _ in 0..5_usize {
        let app_id = app_id.clone();

        // Create a conductor config with the network.
        let path = holochain_conductor_config::generate::generate(
            Some(Default::default()),
            None,
            None,
            false,
            0,
            #[cfg(feature = "chc")]
            None,
        )?;

        // Run a conductor and connect to the admin websocket
        let (admin_port, _, _) =
            run::run_async(&input.holochain_path, path.clone(), None, Output::Log).await?;
        let admin_ws = AdminWebsocket::connect(format!("localhost:{admin_port}"), None).await?;

        let bundle = AppBundleSource::Path(happ.clone()).resolve().await?;
        let bytes = bundle.pack()?;

        // Create the raw InstallAppPayload request.
        let payload = InstallAppPayload {
            installed_app_id: Some(app_id),
            agent_key: None,
            source: AppBundleSource::Bytes(bytes),
            roles_settings: Default::default(),
            network_seed: None,
            ignore_genesis_failure: false,
        };

        let installed_app = admin_ws.install_app(payload).await?;

        admin_ws.enable_app(installed_app.installed_app_id).await?;
    }
    Ok(())
}
