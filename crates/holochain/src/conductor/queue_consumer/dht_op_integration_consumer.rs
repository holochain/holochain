use super::*;
use crate::core::state::workspace::{Workspace, WorkspaceResult};
use holochain_state::env::EnvironmentWrite;
use holochain_state::{
    env::ReadManager,
    prelude::{GetDb, Reader},
};

async fn dht_op_integration_consumer(
    env: EnvironmentWrite,
    rx: Listener,
    mut wake_publish: Waker,
) -> anyhow::Result<()> {
    loop {
        let env_ref = env.guard().await;
        let reader = env_ref.reader()?;
        let writer = OneshotWriter::new(env.clone());
        let workspace = DhtOpIntegrationWorkspace::new(&reader, &env_ref)?;
        dht_op_integration_workflow(workspace, writer, &mut wake_publish).await?;
    }
}

struct DhtOpIntegrationWorkspace<'env>(std::marker::PhantomData<&'env ()>);

impl<'env> DhtOpIntegrationWorkspace<'env> {
    /// Constructor
    #[allow(dead_code)]
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self(std::marker::PhantomData))
    }
}

impl<'env> Workspace<'env> for DhtOpIntegrationWorkspace<'env> {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        todo!()
    }
}

async fn dht_op_integration_workflow<'env>(
    workspace: DhtOpIntegrationWorkspace<'env>,
    writer: OneshotWriter,
    wake_publish: &mut Waker,
) -> anyhow::Result<()> {
    // do stuff

    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;
    let _ = wake_publish.wake();
    Ok(())
}
