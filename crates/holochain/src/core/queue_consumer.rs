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
//! | DhtOpIntegr.   | IntegrationLimbo | IntegratedDhtOps | SysVal + VR    |
//! | ValReceipt.    | IntegratedDhtOps | IntegratedDhtOps | *n/a           |
//! | Publish        | AuthoredDhtOps   | *n/a*            | *n/a*          |
//!
//! († Auth'd + IntQ is short for: AuthoredDhtOps + IntegrationLimbo)
//!
//! Implicitly, every workflow also writes to its own source queue, i.e. to
//! remove the item it has just processed.

use derive_more::Constructor;
use derive_more::Display;
use derive_more::From;
use futures::future::Either;
use holochain_sqlite::db::WriteManager;
use holochain_sqlite::prelude::Writer;
use holochain_types::prelude::*;
use holochain_zome_types::CellId;
use tokio::sync;
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
use validation_receipt_consumer::*;
mod validation_receipt_consumer;
use crate::conductor::{api::CellConductorApiT, error::ConductorError, manager::ManagedTaskResult};
use crate::conductor::{manager::ManagedTaskAdd, ConductorHandle};
use holochain_p2p::*;
use holochain_state::workspace::WorkspaceError;
use publish_dht_ops_consumer::*;

use super::workflow::error::WorkflowError;

/// Spawns several long-running tasks which are responsible for processing work
/// which shows up on various databases.
///
/// Waits for the initial loop to complete before returning, to prevent causing
/// a race condition by trying to run a workflow too soon after cell creation.
pub async fn spawn_queue_consumer_tasks(
    env: &EnvWrite,
    cell_network: HolochainP2pCell,
    conductor_handle: ConductorHandle,
    conductor_api: impl CellConductorApiT + 'static,
    task_sender: sync::mpsc::Sender<ManagedTaskAdd>,
    stop: sync::broadcast::Sender<()>,
) -> (QueueTriggers, InitialQueueTriggers) {
    let cell_id = cell_network.cell_id();
    // Publish
    let (tx_publish, handle) = spawn_publish_dht_ops_consumer(
        env.clone(),
        conductor_handle.clone(),
        stop.subscribe(),
        cell_network.clone(),
    );
    task_sender
        .send(ManagedTaskAdd::cell_critical(
            handle,
            cell_id.clone(),
            "publish_dht_ops_consumer",
        ))
        .await
        .expect("Failed to manage workflow handle");

    // Validation Receipt
    let (tx_receipt, handle) = spawn_validation_receipt_consumer(
        env.clone(),
        conductor_handle.clone(),
        stop.subscribe(),
        cell_network.clone(),
    );
    task_sender
        .send(ManagedTaskAdd::cell_critical(
            handle,
            cell_id.clone(),
            "validation_receipt_consumer",
        ))
        .await
        .expect("Failed to manage workflow handle");

    let (create_tx_sys, get_tx_sys) = tokio::sync::oneshot::channel();

    // Integration
    let (tx_integration, handle) = spawn_integrate_dht_ops_consumer(
        env.clone(),
        conductor_handle.clone(),
        cell_network.cell_id(),
        stop.subscribe(),
        get_tx_sys,
        tx_receipt.clone(),
    );
    task_sender
        .send(ManagedTaskAdd::cell_critical(
            handle,
            cell_id.clone(),
            "integrate_dht_ops_consumer",
        ))
        .await
        .expect("Failed to manage workflow handle");

    // App validation
    let (tx_app, handle) = spawn_app_validation_consumer(
        env.clone(),
        conductor_handle.clone(),
        stop.subscribe(),
        tx_integration.clone(),
        conductor_api.clone(),
        cell_network.clone(),
    );
    task_sender
        .send(ManagedTaskAdd::cell_critical(
            handle,
            cell_id.clone(),
            "app_validation_consumer",
        ))
        .await
        .expect("Failed to manage workflow handle");

    // Sys validation
    let (tx_sys, handle) = spawn_sys_validation_consumer(
        env.clone(),
        conductor_handle.clone(),
        stop.subscribe(),
        tx_app.clone(),
        cell_network.clone(),
        conductor_api,
    );
    task_sender
        .send(ManagedTaskAdd::cell_critical(
            handle,
            cell_id.clone(),
            "sys_validation_consumer",
        ))
        .await
        .expect("Failed to manage workflow handle");
    if create_tx_sys.send(tx_sys.clone()).is_err() {
        panic!("Failed to send tx_sys");
    }

    // Produce
    let (tx_produce, handle) = spawn_produce_dht_ops_consumer(
        env.clone(),
        conductor_handle.clone(),
        cell_network.cell_id(),
        stop.subscribe(),
        tx_publish.clone(),
    );
    task_sender
        .send(ManagedTaskAdd::cell_critical(
            handle,
            cell_id,
            "produce_dht_ops_consumer",
        ))
        .await
        .expect("Failed to manage workflow handle");

    (
        QueueTriggers::new(tx_sys.clone(), tx_produce.clone()),
        InitialQueueTriggers::new(
            tx_sys,
            tx_produce,
            tx_publish,
            tx_app,
            tx_integration,
            tx_receipt,
        ),
    )
}

#[derive(Clone)]
/// The entry points for kicking off a chain reaction of queue activity
pub struct QueueTriggers {
    /// Notify the SysValidation workflow to run, i.e. after handling gossip
    pub sys_validation: TriggerSender,
    /// Notify the ProduceDhtOps workflow to run, i.e. after InvokeCallZome
    pub produce_dht_ops: TriggerSender,
}

/// The triggers to run once at the start of a cell
pub struct InitialQueueTriggers {
    /// These triggers can only be run once
    /// so they are private
    sys_validation: TriggerSender,
    produce_dht_ops: TriggerSender,
    publish_dht_ops: TriggerSender,
    app_validation: TriggerSender,
    integrate_dht_ops: TriggerSender,
    validation_receipt: TriggerSender,
}

impl QueueTriggers {
    /// Create a new queue trigger
    pub fn new(sys_validation: TriggerSender, produce_dht_ops: TriggerSender) -> Self {
        Self {
            sys_validation,
            produce_dht_ops,
        }
    }
}

impl InitialQueueTriggers {
    fn new(
        sys_validation: TriggerSender,
        produce_dht_ops: TriggerSender,
        publish_dht_ops: TriggerSender,
        app_validation: TriggerSender,
        integrate_dht_ops: TriggerSender,
        validation_receipt: TriggerSender,
    ) -> Self {
        Self {
            sys_validation,
            produce_dht_ops,
            publish_dht_ops,
            app_validation,
            integrate_dht_ops,
            validation_receipt,
        }
    }

    /// Initialize all the workflows once.
    pub fn initialize_workflows(mut self) {
        self.sys_validation.trigger();
        self.app_validation.trigger();
        self.publish_dht_ops.trigger();
        self.integrate_dht_ops.trigger();
        self.produce_dht_ops.trigger();
        self.validation_receipt.trigger();
    }
}
/// The means of nudging a queue consumer to tell it to look for more work
#[derive(Clone)]
pub struct TriggerSender(mpsc::Sender<()>);

/// The receiving end of a queue trigger channel
pub struct TriggerReceiver {
    rx: mpsc::Receiver<()>,
    waker: core::task::Waker,
}

impl TriggerSender {
    /// Create a new channel for waking a consumer
    ///
    /// The channel buffer is set to num_cpus to deal with the potential
    /// inconsistency from the perspective of any particular CPU thread
    pub fn new() -> (TriggerSender, TriggerReceiver) {
        let (tx, rx) = mpsc::channel(num_cpus::get());
        let waker = futures::task::noop_waker();
        (TriggerSender(tx), TriggerReceiver { rx, waker })
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
            Err(mpsc::error::TrySendError::Full(_)) => {}
            Ok(()) => {}
        };
    }
}

impl TriggerReceiver {
    /// Listen for one or more items to come through, draining the channel
    /// each time. Bubble up errors on empty channel.
    pub async fn listen(&mut self) -> Result<(), QueueTriggerClosedError> {
        use core::task::Poll;

        // wait for next item
        if self.rx.recv().await.is_some() {
            // drain the channel
            let mut ctx = core::task::Context::from_waker(&self.waker);
            loop {
                match self.rx.poll_recv(&mut ctx) {
                    Poll::Ready(None) => return Err(QueueTriggerClosedError),
                    Poll::Pending => return Ok(()),
                    Poll::Ready(Some(())) => {}
                }
            }
        } else {
            Err(QueueTriggerClosedError)
        }
    }
}

/// A lazy Writer factory which can only be used once.
///
/// This is a way of encapsulating an EnvWrite so that it can only be
/// used to create a single Writer before being consumed.
#[derive(Constructor, From)]
pub struct OneshotWriter(EnvWrite);

impl OneshotWriter {
    /// Create the writer and pass it into a closure.
    pub fn with_writer<F>(self, f: F) -> Result<(), WorkspaceError>
    where
        F: FnOnce(&mut Writer) -> Result<(), WorkspaceError> + Send,
    {
        let mut conn = self.0.conn()?;
        conn.with_commit::<WorkspaceError, (), _>(|w| {
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

/// Inform a workflow to run a job or shutdown
enum Job {
    Run,
    Shutdown,
}

/// Wait for the next job or exit command
async fn next_job_or_exit(
    rx: &mut TriggerReceiver,
    stop: &mut sync::broadcast::Receiver<()>,
) -> Job {
    if stop.try_recv().is_ok() {
        return Job::Shutdown;
    }
    // Check for shutdown or next job
    let next_job = rx.listen();
    let kill = stop.recv();
    tokio::pin!(next_job);
    tokio::pin!(kill);

    if let Either::Left((Err(_), _)) | Either::Right((_, _)) =
        futures::future::select(next_job, kill).await
    {
        Job::Shutdown
    } else {
        Job::Run
    }
}

/// Does nothing.
async fn handle_workflow_error(
    _conductor: ConductorHandle,
    _cell_id: CellId,
    err: WorkflowError,
    _reason: &str,
) -> ManagedTaskResult {
    Err(ConductorError::from(err).into())
}
