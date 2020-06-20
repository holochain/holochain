//! Manages the spawning of tasks which process the various work queues in
//! the system, as well as notifying subsequent queue processors to pick up the
//! work that was left off.
//!
//! The following table lays out the queues and the workflows that consume them,
//! as well as the follow-up workflows. A "source" queue is a database which
//! feeds data to the workflow, and a "destination" queue is a database which
//! said workflow writes to as part of its processing of its source queue.
//!
//! | workflow       | source queue     | dest. queue      | notifies       |
//! |----------------|------------------|------------------|----------------|
//! |                        **gossip path**                                |
//! | HandleGossip   | *n/a*            | ValidationQueue  | SysValidation  |
//! | SysValidation  | ValidationQueue  | ValidationQueue  | AppValidation  |
//! | AppValidation  | ValidationQueue  | ValidationQueue  | DhtOpIntegr.   |
//! |                       **authoring path**                              |
//! | CallZome       | *n/a*            | ChainSequence    | ProduceDhtOps  |
//! | ProduceDhtOps  | ChainSequence    | Auth'd + IntQ †  | DhtOpIntegr.   |
//! |                 **integration, common to both paths**                 |
//! | DhtOpIntegr.   | IntegrationQueue | IntegratedDhtOps | Publish        |
//! | Publish        | AuthoredDhtOps   | *n/a*            | *n/a*          |
//!
//! († Auth'd + IntQ is short for: AuthoredDhtOps + IntegrationQueue)
//!
//! Implicitly, every workflow also writes to its own source queue, i.e. to
//! remove the item it has just processed.

use derive_more::{Constructor, Display, From};
use holochain_state::{
    env::{EnvironmentWrite, WriteManager},
    error::DatabaseError,
    prelude::Writer,
};
use tokio::sync::mpsc;

// TODO: move these to workflow mod
mod integrate_dht_ops_consumer;
use integrate_dht_ops_consumer::*;
mod sys_validation_consumer;
use sys_validation_consumer::*;
mod app_validation_consumer;
use app_validation_consumer::*;
mod produce_dht_ops_consumer;
use produce_dht_ops_consumer::*;
mod publish_dht_ops_consumer;
use publish_dht_ops_consumer::*;

/// Spawns several long-running tasks which are responsible for processing work
/// which shows up on various databases.
pub fn spawn_queue_consumer_tasks(env: &EnvironmentWrite) -> InitialQueueTriggers {
    let (tx_publish_dht_ops, _) = spawn_publish_dht_ops_consumer(env.clone());
    let (tx_integration, _) = spawn_integrate_dht_ops_consumer(env.clone(), tx_publish_dht_ops);
    let (tx_app_validation, _) = spawn_app_validation_consumer(env.clone(), tx_integration.clone());
    let (tx_sys_validation, _) = spawn_sys_validation_consumer(env.clone(), tx_app_validation);
    let (tx_produce_dht_ops, _) = spawn_produce_dht_ops_consumer(env.clone(), tx_integration);

    InitialQueueTriggers {
        sys_validation: tx_sys_validation,
        produce_dht_ops: tx_produce_dht_ops,
    }
}

/// The entry points for kicking off a chain reaction of queue activity
pub struct InitialQueueTriggers {
    /// Notify the SysValidation workflow to run, i.e. after handling gossip
    pub sys_validation: QueueTrigger,
    /// Notify the ProduceDhtOps workflow to run, i.e. after InvokeCallZome
    pub produce_dht_ops: QueueTrigger,
}

/// The means of nudging a queue consumer to tell it to look for more work
#[derive(Clone)]
pub struct QueueTrigger(mpsc::Sender<()>);

impl QueueTrigger {
    /// Create a new channel for waking a consumer
    ///
    /// The channel buffer is set to 1 to ensure that the consumer does not
    /// have to be concerned with draining the channel in case it has received
    /// multiple trigger signals.
    pub fn new() -> (Self, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel(1);
        (Self(tx), rx)
    }

    /// Lazily nudge the consumer task, ignoring the case where the consumer
    /// already has a pending trigger signal
    pub fn trigger(&mut self) -> Result<(), QueueTriggerClosedError> {
        match self.0.try_send(()) {
            Err(mpsc::error::TrySendError::Closed(_)) => Err(QueueTriggerClosedError),
            Err(mpsc::error::TrySendError::Full(_)) => Ok(()),
            Ok(()) => Ok(()),
        }
    }
}

#[derive(Constructor, From)]
pub struct OneshotWriter(EnvironmentWrite);

impl OneshotWriter {
    pub async fn with_writer<F>(self, f: F) -> Result<(), DatabaseError>
    where
        F: FnOnce(&mut Writer) -> () + Send,
    {
        let env_ref = self.0.guard().await;
        env_ref.with_commit::<DatabaseError, (), _>(|w| {
            f(w);
            Ok(())
        })?;
        Ok(())
    }
}

/// Declares whether a workflow has exhausted the queue or not
#[derive(Clone, Debug, PartialEq)]
pub enum WorkComplete {
    Complete,
    Incomplete,
}

/// The only error possible when attempting to trigger: the channel is closed
#[derive(Debug, Display, thiserror::Error)]
pub struct QueueTriggerClosedError;

/*
/// experimental DRY struct that doesn't work very well, or at all

pub struct QueueConsumer<Ws>
where
    Ws: for<'env> Workspace<'env>,
{
    env: EnvironmentWrite,
    run_consumer:
        Box<dyn Fn(Ws) -> MustBoxFuture<'static, WorkflowResult<WorkComplete>> + Send + Sync>,
    channel: (QueueTrigger, QueueTriggerListener),
    // triggers: Vec<QueueTrigger>,
}

impl<Ws> QueueConsumer<Ws>
where
    Ws: for<'env> Workspace<'env>,
{
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let (trigger_self, rx) = self.channel;
            loop {
                let env_ref = self.env.guard().await;
                let reader = env_ref.reader().expect("Could not create LMDB reader");
                let workspace =
                    produce_dht_opsWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
                if let WorkComplete::Incomplete = (*self.run_consumer)(workspace)
                    .await
                    .expect("Failed to run workflow")
                {
                    trigger_self.trigger().expect("Trigger channel closed")
                };
                rx.next().await;
            }
        })
    }
}
*/
