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
//! | DhtOpIntegr.   | IntegrationLimbo | IntegratedDhtOps | Publish        |
//! | Publish        | AuthoredDhtOps   | *n/a*            | *n/a*          |
//!
//! († Auth'd + IntQ is short for: AuthoredDhtOps + IntegrationLimbo)
//!
//! Implicitly, every workflow also writes to its own source queue, i.e. to
//! remove the item it has just processed.

use derive_more::{Constructor, Display, From};
use holochain_state::{
    env::{EnvironmentWrite, WriteManager},
    prelude::Writer,
};
use tokio::sync::{self, mpsc};

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
use super::state::workspace::WorkspaceError;
use crate::conductor::manager::ManagedTaskAdd;
use holochain_p2p::HolochainP2pCell;
use publish_dht_ops_consumer::*;

/// Spawns several long-running tasks which are responsible for processing work
/// which shows up on various databases.
///
/// Waits for the initial loop to complete before returning, to prevent causing
/// a race condition by trying to run a workflow too soon after cell creation.
pub async fn spawn_queue_consumer_tasks(
    env: &EnvironmentWrite,
    cell_network: HolochainP2pCell,
    mut task_sender: sync::mpsc::Sender<ManagedTaskAdd>,
    stop: sync::broadcast::Sender<()>,
) -> InitialQueueTriggers {
    let (tx_publish, rx1, handle) =
        spawn_publish_dht_ops_consumer(env.clone().into(), stop.subscribe(), cell_network);
    task_sender
        .send(ManagedTaskAdd::dont_handle(handle))
        .await
        .expect("Failed to manage workflow handle");
    let (tx_integration, rx2, handle) =
        spawn_integrate_dht_ops_consumer(env.clone().into(), stop.subscribe(), tx_publish);
    task_sender
        .send(ManagedTaskAdd::dont_handle(handle))
        .await
        .expect("Failed to manage workflow handle");
    let (tx_app, rx3, handle) =
        spawn_app_validation_consumer(env.clone().into(), stop.subscribe(), tx_integration.clone());
    task_sender
        .send(ManagedTaskAdd::dont_handle(handle))
        .await
        .expect("Failed to manage workflow handle");
    let (tx_sys, rx4, handle) =
        spawn_sys_validation_consumer(env.clone().into(), stop.subscribe(), tx_app);
    task_sender
        .send(ManagedTaskAdd::dont_handle(handle))
        .await
        .expect("Failed to manage workflow handle");
    let (tx_produce, rx5, handle) = spawn_produce_dht_ops_consumer(
        env.clone().into(),
        stop.subscribe(),
        tx_integration.clone(),
    );
    task_sender
        .send(ManagedTaskAdd::dont_handle(handle))
        .await
        .expect("Failed to manage workflow handle");

    // Wait for initial loop to complete for each consumer
    futures::future::join_all(vec![rx1, rx2, rx3, rx4, rx5].into_iter())
        .await
        .into_iter()
        .collect::<Result<Vec<()>, _>>()
        .expect("A queue consumer's oneshot channel was closed before initializing.");

    InitialQueueTriggers {
        sys_validation: tx_sys,
        produce_dht_ops: tx_produce,
    }
}

/// The entry points for kicking off a chain reaction of queue activity
pub struct InitialQueueTriggers {
    /// Notify the SysValidation workflow to run, i.e. after handling gossip
    pub sys_validation: TriggerSender,
    /// Notify the ProduceDhtOps workflow to run, i.e. after InvokeCallZome
    pub produce_dht_ops: TriggerSender,
}

/// The means of nudging a queue consumer to tell it to look for more work
#[derive(Clone)]
pub struct TriggerSender(mpsc::Sender<()>);

/// The receiving end of a queue trigger channel
pub struct TriggerReceiver(mpsc::Receiver<()>);

impl TriggerSender {
    /// Create a new channel for waking a consumer
    ///
    /// The channel buffer is set to num_cpus to deal with the potential
    /// inconsistency from the perspective of any particular CPU thread
    pub fn new() -> (TriggerSender, TriggerReceiver) {
        let (tx, rx) = mpsc::channel(num_cpus::get());
        (TriggerSender(tx), TriggerReceiver(rx))
    }

    /// Lazily nudge the consumer task, ignoring the case where the consumer
    /// already has a pending trigger signal
    pub fn trigger(&mut self) {
        match self.0.try_send(()) {
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::warn!(
                    "Queue consumer trigger was sent while Cell is shutting down: ignoring."
                );
            }
            Err(mpsc::error::TrySendError::Full(_)) => (),
            Ok(()) => (),
        };
    }
}

impl TriggerReceiver {
    /// Listen for one or more items to come through, draining the channel
    /// each time. Bubble up errors on empty channel.
    pub async fn listen(&mut self) -> Result<(), QueueTriggerClosedError> {
        use tokio::sync::mpsc::error::TryRecvError;

        // wait for next item
        if let Some(_) = self.0.recv().await {
            // drain the channel
            loop {
                match self.0.try_recv() {
                    Err(TryRecvError::Closed) => return Err(QueueTriggerClosedError),
                    Err(TryRecvError::Empty) => return Ok(()),
                    Ok(()) => (),
                }
            }
        } else {
            return Err(QueueTriggerClosedError);
        }
    }
}

/// A lazy Writer factory which can only be used once.
///
/// This is a way of encapsulating an EnvironmentWrite so that it can only be
/// used to create a single Writer before being consumed.
#[derive(Constructor, From)]
pub struct OneshotWriter(EnvironmentWrite);

impl OneshotWriter {
    /// Create the writer and pass it into a closure.
    pub async fn with_writer<F>(self, f: F) -> Result<(), WorkspaceError>
    where
        F: FnOnce(&mut Writer) -> Result<(), WorkspaceError> + Send,
    {
        let env_ref = self.0.guard().await;
        env_ref.with_commit::<WorkspaceError, (), _>(|w| {
            f(w)?;
            Ok(())
        })?;
        Ok(())
    }
}

/// Declares whether a workflow has exhausted the queue or not
#[derive(Clone, Debug, PartialEq)]
pub enum WorkComplete {
    /// The queue has been exhausted
    Complete,
    /// Items still remain on the queue
    Incomplete,
}

/// The only error possible when attempting to trigger: the channel is closed
#[derive(Debug, Display, thiserror::Error)]
pub struct QueueTriggerClosedError;
