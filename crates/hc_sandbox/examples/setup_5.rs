use std::path::PathBuf;

use hc_sandbox::calls::EnableApp;
use hc_sandbox::expect_match;
use hc_sandbox::CmdRunner;
use holochain_cli_sandbox as hc_sandbox;
use holochain_conductor_api::AdminRequest;
use holochain_conductor_api::AdminResponse;
use holochain_p2p::kitsune_p2p::KitsuneP2pConfig;
use holochain_types::prelude::AppBundleSource;
use holochain_types::prelude::InstallAppPayload;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Input {
    #[structopt(short, long, default_value = "holochain")]
    holochain_path: PathBuf,
    happ: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get and parse any input.
    let input = Input::from_args();
    let happ = hc_sandbox::bundles::parse_happ(input.happ)?;

    // Using the default mem network.
    let network = KitsuneP2pConfig::default();

    // Choose an app id and properties.
    let app_id = "my-cool-app".to_string();

    for _ in 0..5 as usize {
        let app_id = app_id.clone();

        // Create a conductor config with the network.
        let path = hc_sandbox::generate::generate(Some(network.clone()), None, None)?;

        // Create a command runner to run admin commands.
        // This runs the conductor in the background and cleans
        // up the process when the guard is dropped.
        let (mut cmd, _conductor_guard) =
            CmdRunner::from_sandbox_with_bin_path(&input.holochain_path, path.clone()).await?;

        // Generate a new agent key using the simple calls api.
        let agent_key = hc_sandbox::calls::generate_agent_pub_key(&mut cmd).await?;

        let bundle = AppBundleSource::Path(happ.clone()).resolve().await?;

        // Create the raw InstallAppPayload request.
        let payload = InstallAppPayload {
            installed_app_id: Some(app_id),
            agent_key,
            source: AppBundleSource::Bundle(bundle),
            membrane_proofs: Default::default(),
            network_seed: None,
        };

        let r = AdminRequest::InstallApp(Box::new(payload));

        // Run the command and wait for the response.
        let installed_app = cmd.command(r).await?;

        // Check you got the correct response and get the inner value.
        let installed_app = expect_match!(installed_app => AdminResponse::AppInstalled, "Failed to install app");

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
