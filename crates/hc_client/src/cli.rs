use clap::{Parser, Subcommand};

use crate::{calls, membrane_proofs, zome_call};

/// Client commands that can be executed.
#[derive(Debug, Subcommand)]
pub enum ClientCommand {
    /// Invoke an admin API request via the conductor's admin interface.
    Call(calls::Call),
    /// Generate signing credentials for zome calls and grant capabilities.
    #[command(name = "zome-call-auth")]
    ZomeCallAuth(zome_call::ZomeCallAuth),
    /// Make a zome call against a running conductor.
    #[command(name = "zome-call")]
    ZomeCall(zome_call::ZomeCall),
    /// Provide membrane proofs to an app installed with deferred proofs.
    #[command(name = "provide-memproofs")]
    ProvideMemproofs(membrane_proofs::ProvideMemproofs),
}

/// Execution context for running CLI commands.
#[derive(Debug, Parser)]
#[command(
    name = "client",
    about = "Connect to and interact with running Holochain conductors",
    author,
    version
)]
pub struct HcClient {
    /// Command to execute.
    #[command(subcommand)]
    pub command: ClientCommand,
}

impl HcClient {
    /// Run this command (used by the hc CLI).
    pub async fn run(self) -> anyhow::Result<()> {
        match self.command {
            ClientCommand::Call(call) => calls::call(call).await,
            ClientCommand::ZomeCallAuth(auth) => zome_call::zome_call_auth(auth).await,
            ClientCommand::ZomeCall(call) => zome_call::zome_call(call).await,
            ClientCommand::ProvideMemproofs(args) => membrane_proofs::provide_memproofs(args).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HcClient;
    use clap::Parser;

    #[test]
    fn deferred_membrane_proofs_command_accepts_file_flags() {
        let parsed = HcClient::try_parse_from([
            "hc-client",
            "provide-memproofs",
            "--port",
            "12345",
            "my-app",
            "--membrane-proof",
            "role-1=proof.bin",
        ]);
        assert!(parsed.is_ok(), "{parsed:?}");
    }
}
