use std::path::PathBuf;

/*use hc_sandbox::calls::ActivateApp;
use hc_sandbox::expect_match;
use hc_sandbox::CmdRunner;
use holochain_cli_sandbox as hc_sandbox;
use holochain_conductor_api::AdminRequest;
use holochain_conductor_api::AdminResponse;
use holochain_p2p::kitsune_p2p::KitsuneP2pConfig;
use holochain_types::prelude::InstallAppDnaPayload;
use holochain_types::prelude::InstallAppPayload;
use holochain_types::prelude::YamlProperties;
 */

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Input {
    #[structopt(short, long, default_value = "holochain")]
    holochain_path: PathBuf,
    dnas: Vec<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    todo!("Fix the way this gets installed");
    /*
        // Get and parse any input.
        let input = Input::from_args();
        let dnas = hc_sandbox::dna::parse_dnas(input.dnas)?;

        // Using the default mem network.
        let network = KitsuneP2pConfig::default();

        // Choose an app id and properties.
        let app_id = "my-cool-app".to_string();
        let properties = Some(YamlProperties::new(serde_yaml::Value::String(
            "my-cool-property".to_string(),
        )));

        for _ in 0..5 as usize {
            let app_id = app_id.clone();
            let properties = properties.clone();

            // Create a conductor config with the network.
            let path = hc_sandbox::generate::generate(Some(network.clone()), None, None)?;

            // Create a command runner to run admin commands.
            // This runs the conductor in the background and cleans
            // up the process when the guard is dropped.
            let (mut cmd, _conductor_guard) =
                CmdRunner::from_sandbox_with_bin_path(&input.holochain_path, path.clone()).await?;

            // Generate a new agent key using the simple calls api.
            let agent_key = hc_sandbox::calls::generate_agent_pub_key(&mut cmd).await?;

            // Turn dnas into payloads.
            let dnas = dnas
                .clone()
                .into_iter()
                .enumerate()
                .map(|(i, path)| {
                    // Create an app for this dna with app id and the dna position.
                    let mut payload =
                        InstallAppDnaPayload::path_only(path, format!("{}-{}", app_id, i));
                    // Add the properties.
                    payload.properties = properties.clone();
                    payload
                })
                .collect::<Vec<_>>();

            // This is an example of the lower level call to the admin interface.

            // Create the admin request.
            // This is the same type that is used for
            // anyone calling the admin api.
            let app = InstallAppPayload {
                installed_app_id: app_id,
                agent_key,
                dnas,
            };
            let r = AdminRequest::InstallApp(app.into());

            // Run the command and wait for the response.
            let installed_app = cmd.command(r).await?;

            // Check you got the correct response and get the inner value.
            let installed_app =
                expect_match!(installed_app => AdminResponse::AppInstalled, "Failed to install app");

            // Activate the app using the simple calls api.
            hc_sandbox::calls::activate_app(
                &mut cmd,
                ActivateApp {
                    app_id: installed_app.installed_app_id().clone(),
                },
            )
            .await?;
        }
        Ok(())
    */
}
