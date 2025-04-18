use std::path::PathBuf;

use hc_sandbox::calls::EnableApp;
use hc_sandbox::expect_match;
use hc_sandbox::CmdRunner;
use holochain_cli_sandbox as hc_sandbox;
use holochain_conductor_api::AdminRequest;
use holochain_conductor_api::AdminResponse;
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

        // Create a command runner to run admin commands.
        // This runs the conductor in the background and cleans
        // up the process when the guard is dropped.
        let (mut cmd, _conductor_guard) =
            CmdRunner::from_sandbox_with_bin_path(&input.holochain_path, path.clone()).await?;

        let bundle = AppBundleSource::Path(happ.clone()).resolve().await?;
        let bytes = bundle.encode()?;

        // Create the raw InstallAppPayload request.
        let payload = InstallAppPayload {
            installed_app_id: Some(app_id),
            agent_key: None,
            source: AppBundleSource::Bytes(bytes),
            roles_settings: Default::default(),
            network_seed: None,
            ignore_genesis_failure: false,
        };

        let r = AdminRequest::InstallApp(Box::new(payload));

        // Run the command and wait for the response.
        let installed_app = cmd.command(r).await?;

        // Check you got the correct response and get the inner value.
        let installed_app =
            expect_match!(installed_app => AdminResponse::AppInstalled, "Failed to install app");

        // Activate the app using the simple calls api.
        hc_sandbox::calls::enable_app(
            &mut cmd,
            EnableApp {
                app_id: installed_app.installed_app_id,
            },
        )
        .await?;
    }
    Ok(())
}
