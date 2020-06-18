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

use derive_more::{Constructor, From};
use dht_op_integration_workflow::spawn_dht_op_integration_consumer;
use holochain_state::{
    env::{EnvironmentRefRw, EnvironmentWrite, WriteManager},
    error::DatabaseError,
    prelude::Writer,
};
use publish_workflow::spawn_publish_consumer;
use tokio::sync::mpsc;

// TODO: move these to workflow mod
mod dht_op_integration_workflow;
use dht_op_integration_workflow::*;
mod sys_validation_workflow;
use sys_validation_workflow::*;
mod app_validation_workflow;
use app_validation_workflow::*;
mod produce_workflow;
use produce_workflow::*;
mod publish_workflow;
use publish_workflow::*;

/// Spawns several long-running tasks which are responsible for processing work
/// which shows up on various databases.
pub async fn spawn_queue_consumer_tasks(env: EnvironmentWrite) {
    // TODO: sys validation is not triggered until HandleGossip workflow
    // is implemented
    let (tx_sys_validation, rx_sys_validation) = QueueTrigger::new();
    let (tx_app_validation, rx_app_validation) = QueueTrigger::new();
    let (tx_produce, rx_produce) = QueueTrigger::new();
    let (tx_integration, rx_integration) = QueueTrigger::new();
    let (tx_publish, rx_publish) = QueueTrigger::new();

    spawn_sys_validation_consumer(env.clone(), rx_sys_validation, tx_app_validation);
    spawn_app_validation_consumer(env.clone(), rx_app_validation, tx_integration.clone());
    spawn_produce_consumer(env.clone(), rx_produce, tx_integration);
    spawn_dht_op_integration_consumer(env.clone(), rx_integration, tx_publish);
    spawn_publish_consumer(env.clone(), rx_publish);
}

/// The means of nudging a queue consumer to tell it to look for more work
#[derive(Clone)]
pub struct QueueTrigger(mpsc::Sender<()>);

/// The receiving side of a QueueTrigger channel
type QueueTriggerListener = mpsc::Receiver<()>;

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
struct OneshotWriter(EnvironmentWrite);

impl OneshotWriter {
    pub async fn with_writer<F>(self, f: F) -> Result<(), DatabaseError>
    where
        F: FnOnce(&mut Writer) -> () + Send,
    {
        let env_ref = self.0.guard().await;
        env_ref.with_commit::<DatabaseError, (), _>(|w| Ok(f(w)))?;
        Ok(())
    }
}

/// The only error possible when attempting to trigger: the channel is closed
pub struct QueueTriggerClosedError;
