use holochain_cli_client::{cli::ClientCli, Context};
use tokio::runtime::Runtime;

fn main() -> anyhow::Result<()> {
    let cli = ClientCli::parse_from(std::env::args());
    Runtime::new()?.block_on(Context::default().execute(cli))
}
