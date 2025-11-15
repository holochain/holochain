use crate::cli::{ClientCli, ClientCommand};
use crate::opts::ClientOptions;

#[derive(Default)]
pub struct Context;

impl Context {
    pub async fn execute(self, cli: ClientCli) -> anyhow::Result<()> {
        let opts = ClientOptions::from_cli(&cli);
        match cli.command {
            ClientCommand::Call(args) => crate::commands::admin_call(&opts, args).await,
            ClientCommand::ZomeCallAuth(args) => crate::commands::zome_call_auth(&opts, args).await,
            ClientCommand::ZomeCall(args) => crate::commands::zome_call(&opts, args).await,
        }
    }
}
