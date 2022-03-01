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

use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use derive_more::Display;
use futures::future::Either;
use holochain_types::prelude::*;
use holochain_zome_types::CellId;
use tokio::sync::{self, broadcast};

// MAYBE: move these to workflow mod
mod integrate_dht_ops_consumer;
use integrate_dht_ops_consumer::*;
mod sys_validation_consumer;
use sys_validation_consumer::*;
mod app_validation_consumer;
use app_validation_consumer::*;
mod publish_dht_ops_consumer;
use tokio::task::JoinHandle;
use validation_receipt_consumer::*;
mod validation_receipt_consumer;
use crate::conductor::conductor::RwShare;
use crate::conductor::space::Space;
use crate::conductor::{error::ConductorError, manager::ManagedTaskResult};
use crate::conductor::{manager::ManagedTaskAdd, ConductorHandle};
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::*;
use publish_dht_ops_consumer::*;

mod countersigning_consumer;
use countersigning_consumer::*;

#[cfg(test)]
mod tests;

use super::workflow::app_validation_workflow::AppValidationWorkspace;
use super::workflow::error::WorkflowError;
use super::workflow::sys_validation_workflow::SysValidationWorkspace;

/// Spawns several long-running tasks which are responsible for processing work
/// which shows up on various databases.
///
/// Waits for the initial loop to complete before returning, to prevent causing
/// a race condition by trying to run a workflow too soon after cell creation.
#[allow(clippy::too_many_arguments)]
pub async fn spawn_queue_consumer_tasks(
    cell_id: CellId,
    network: HolochainP2pDna,
    space: &Space,
    conductor_handle: ConductorHandle,
    task_sender: sync::mpsc::Sender<ManagedTaskAdd>,
    stop: sync::broadcast::Sender<()>,
) -> (QueueTriggers, InitialQueueTriggers) {
    let Space {
        authored_env,
        dht_env,
        cache,
        dht_query_cache,
        ..
    } = space;

    let keystore = conductor_handle.keystore().clone();
    let dna_hash = Arc::new(cell_id.dna_hash().clone());
    let queue_consumer_map = conductor_handle.get_queue_consumer_workflows();

    // Publish
    let (tx_publish, handle) = spawn_publish_dht_ops_consumer(
        cell_id.agent_pubkey().clone(),
        authored_env.clone(),
        conductor_handle.clone(),
        stop.subscribe(),
        Box::new(network.clone()),
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
    // One per space.
    let (tx_receipt, handle) =
        queue_consumer_map.spawn_once_validation_receipt(dna_hash.clone(), || {
            spawn_validation_receipt_consumer(
                dna_hash.clone(),
                dht_env.clone(),
                conductor_handle.clone(),
                stop.subscribe(),
                network.clone(),
            )
        });

    if let Some(handle) = handle {
        task_sender
            .send(ManagedTaskAdd::cell_critical(
                handle,
                cell_id.clone(),
                "validation_receipt_consumer",
            ))
            .await
            .expect("Failed to manage workflow handle");
    }

    // Integration
    // One per space.
    let (tx_integration, handle) =
        queue_consumer_map.spawn_once_integration(dna_hash.clone(), || {
            spawn_integrate_dht_ops_consumer(
                dna_hash.clone(),
                dht_env.clone(),
                dht_query_cache.clone(),
                stop.subscribe(),
                tx_receipt.clone(),
                network.clone(),
            )
        });

    if let Some(handle) = handle {
        task_sender
            .send(ManagedTaskAdd::cell_critical(
                handle,
                cell_id.clone(),
                "integrate_dht_ops_consumer",
            ))
            .await
            .expect("Failed to manage workflow handle");
    }

    let dna_def = conductor_handle
        .get_dna_def(&*dna_hash)
        .expect("Dna must be in store");

    // App validation
    // One per space.
    let (tx_app, handle) = queue_consumer_map.spawn_once_app_validation(dna_hash.clone(), || {
        spawn_app_validation_consumer(
            dna_hash.clone(),
            AppValidationWorkspace::new(
                authored_env.clone().into(),
                dht_env.clone(),
                cache.clone(),
                keystore.clone(),
                Arc::new(dna_def),
            ),
            conductor_handle.clone(),
            stop.subscribe(),
            tx_integration.clone(),
            network.clone(),
            dht_query_cache.clone(),
        )
    });
    if let Some(handle) = handle {
        task_sender
            .send(ManagedTaskAdd::cell_critical(
                handle,
                cell_id.clone(),
                "app_validation_consumer",
            ))
            .await
            .expect("Failed to manage workflow handle");
    }

    let dna_def = conductor_handle
        .get_dna_def(&*dna_hash)
        .expect("Dna must be in store");

    // Sys validation
    // One per space.
    let (tx_sys, handle) = queue_consumer_map.spawn_once_sys_validation(dna_hash.clone(), || {
        spawn_sys_validation_consumer(
            SysValidationWorkspace::new(
                authored_env.clone().into(),
                dht_env.clone().into(),
                dht_query_cache.clone(),
                cache.clone(),
                Arc::new(dna_def),
            ),
            space.clone(),
            conductor_handle.clone(),
            stop.subscribe(),
            tx_app.clone(),
            network.clone(),
        )
    });

    if let Some(handle) = handle {
        task_sender
            .send(ManagedTaskAdd::cell_critical(
                handle,
                cell_id.clone(),
                "sys_validation_consumer",
            ))
            .await
            .expect("Failed to manage workflow handle");
    }

    let (tx_cs, handle) = queue_consumer_map.spawn_once_countersigning(dna_hash.clone(), || {
        spawn_countersigning_consumer(
            space.clone(),
            stop.subscribe(),
            network.clone(),
            tx_sys.clone(),
        )
    });
    if let Some(handle) = handle {
        task_sender
            .send(ManagedTaskAdd::cell_critical(
                handle,
                cell_id.clone(),
                "countersigning_consumer",
            ))
            .await
            .expect("Failed to manage workflow handle");
    }

    (
        QueueTriggers {
            sys_validation: tx_sys.clone(),
            publish_dht_ops: tx_publish.clone(),
            countersigning: tx_cs,
            integrate_dht_ops: tx_integration.clone(),
        },
        InitialQueueTriggers::new(tx_sys, tx_publish, tx_app, tx_integration, tx_receipt),
    )
}

#[derive(Clone)]
/// Map of running queue consumers workflows per dna space.
pub struct QueueConsumerMap {
    map: RwShare<HashMap<QueueEntry, TriggerSender>>,
}

impl Default for QueueConsumerMap {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueConsumerMap {
    /// Create a new queue consumer map.
    pub fn new() -> Self {
        Self {
            map: RwShare::new(HashMap::new()),
        }
    }

    fn spawn_once_validation_receipt<S>(
        &self,
        dna_hash: Arc<DnaHash>,
        spawn: S,
    ) -> (TriggerSender, Option<JoinHandle<ManagedTaskResult>>)
    where
        S: FnOnce() -> (TriggerSender, JoinHandle<ManagedTaskResult>),
    {
        self.spawn_once(QueueEntry(dna_hash, QueueType::Receipt), spawn)
    }

    fn spawn_once_integration<S>(
        &self,
        dna_hash: Arc<DnaHash>,
        spawn: S,
    ) -> (TriggerSender, Option<JoinHandle<ManagedTaskResult>>)
    where
        S: FnOnce() -> (TriggerSender, JoinHandle<ManagedTaskResult>),
    {
        self.spawn_once(QueueEntry(dna_hash, QueueType::Integration), spawn)
    }

    fn spawn_once_sys_validation<S>(
        &self,
        dna_hash: Arc<DnaHash>,
        spawn: S,
    ) -> (TriggerSender, Option<JoinHandle<ManagedTaskResult>>)
    where
        S: FnOnce() -> (TriggerSender, JoinHandle<ManagedTaskResult>),
    {
        self.spawn_once(QueueEntry(dna_hash, QueueType::SysValidation), spawn)
    }

    fn spawn_once_app_validation<S>(
        &self,
        dna_hash: Arc<DnaHash>,
        spawn: S,
    ) -> (TriggerSender, Option<JoinHandle<ManagedTaskResult>>)
    where
        S: FnOnce() -> (TriggerSender, JoinHandle<ManagedTaskResult>),
    {
        self.spawn_once(QueueEntry(dna_hash, QueueType::AppValidation), spawn)
    }

    fn spawn_once_countersigning<S>(
        &self,
        dna_hash: Arc<DnaHash>,
        spawn: S,
    ) -> (TriggerSender, Option<JoinHandle<ManagedTaskResult>>)
    where
        S: FnOnce() -> (TriggerSender, JoinHandle<ManagedTaskResult>),
    {
        self.spawn_once(QueueEntry(dna_hash, QueueType::Countersigning), spawn)
    }

    /// Get the validation receipt trigger for this dna hash.
    pub fn validation_receipt_trigger(&self, dna_hash: Arc<DnaHash>) -> Option<TriggerSender> {
        self.get_trigger(&QueueEntry(dna_hash, QueueType::Receipt))
    }

    /// Get the integration trigger for this dna hash.
    pub fn integration_trigger(&self, dna_hash: Arc<DnaHash>) -> Option<TriggerSender> {
        self.get_trigger(&QueueEntry(dna_hash, QueueType::Integration))
    }

    /// Get the sys validation trigger for this dna hash.
    pub fn sys_validation_trigger(&self, dna_hash: Arc<DnaHash>) -> Option<TriggerSender> {
        self.get_trigger(&QueueEntry(dna_hash, QueueType::SysValidation))
    }

    /// Get the app validation trigger for this dna hash.
    pub fn app_validation_trigger(&self, dna_hash: Arc<DnaHash>) -> Option<TriggerSender> {
        self.get_trigger(&QueueEntry(dna_hash, QueueType::AppValidation))
    }

    /// Get the countersigning trigger for this dna hash.
    pub fn countersigning_trigger(&self, dna_hash: Arc<DnaHash>) -> Option<TriggerSender> {
        self.get_trigger(&QueueEntry(dna_hash, QueueType::Countersigning))
    }

    fn get_trigger(&self, key: &QueueEntry) -> Option<TriggerSender> {
        self.map.share_ref(|map| map.get(key).cloned())
    }

    fn spawn_once<S>(
        &self,
        key: QueueEntry,
        spawn: S,
    ) -> (TriggerSender, Option<JoinHandle<ManagedTaskResult>>)
    where
        S: FnOnce() -> (TriggerSender, JoinHandle<ManagedTaskResult>),
    {
        self.map.share_mut(|map| match map.entry(key) {
            std::collections::hash_map::Entry::Occupied(o) => (o.get().clone(), None),
            std::collections::hash_map::Entry::Vacant(v) => {
                let (ts, handle) = spawn();
                (v.insert(ts).clone(), Some(handle))
            }
        })
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct QueueEntry(Arc<DnaHash>, QueueType);

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
enum QueueType {
    Receipt,
    Integration,
    AppValidation,
    SysValidation,
    Countersigning,
}

/// The entry points for kicking off a chain reaction of queue activity
#[derive(Clone)]
pub struct QueueTriggers {
    /// Notify the SysValidation workflow to run, i.e. after handling gossip
    pub sys_validation: TriggerSender,
    /// Notify the ProduceDhtOps workflow to run, i.e. after InvokeCallZome
    pub publish_dht_ops: TriggerSender,
    /// Notify the countersigning workflow to run, i.e. after receiving
    /// new countersigning data.
    pub countersigning: TriggerSender,
    /// Notify the IntegrateDhtOps workflow to run, i.e. after InvokeCallZome
    pub integrate_dht_ops: TriggerSender,
}

/// The triggers to run once at the start of a cell
#[derive(Clone)]
pub struct InitialQueueTriggers {
    /// These triggers can only be run once
    /// so they are private
    sys_validation: TriggerSender,
    publish_dht_ops: TriggerSender,
    app_validation: TriggerSender,
    integrate_dht_ops: TriggerSender,
    validation_receipt: TriggerSender,
}

impl InitialQueueTriggers {
    fn new(
        sys_validation: TriggerSender,
        publish_dht_ops: TriggerSender,
        app_validation: TriggerSender,
        integrate_dht_ops: TriggerSender,
        validation_receipt: TriggerSender,
    ) -> Self {
        Self {
            sys_validation,
            publish_dht_ops,
            app_validation,
            integrate_dht_ops,
            validation_receipt,
        }
    }

    /// Initialize all the workflows once.
    pub fn initialize_workflows(self) {
        self.sys_validation.trigger();
        self.app_validation.trigger();
        self.integrate_dht_ops.trigger();
        self.publish_dht_ops.trigger();
        self.validation_receipt.trigger();
    }
}
/// The means of nudging a queue consumer to tell it to look for more work
#[derive(Clone)]
pub struct TriggerSender {
    /// The actual trigger sender.
    trigger: broadcast::Sender<()>,
    /// Reset the back off loop if there is one.
    reset_back_off: Option<Arc<AtomicBool>>,
    /// Pause / resume the back off loop if there is one.
    pause_back_off: Option<Arc<AtomicBool>>,
}

/// The receiving end of a queue trigger channel
pub struct TriggerReceiver {
    /// The actual trigger.
    rx: broadcast::Receiver<()>,
    /// If there is a back off loop, should
    /// the trigger reset the back off.
    reset_on_trigger: bool,
    /// The optional back off loop.
    back_off: Option<BackOff>,
}

/// A loop that can optionally back off, pause and resume.
struct BackOff {
    /// The starting duration for the back off.
    /// This allows resetting the range.
    start: Duration,
    /// The range of duration for the back off.
    range: Range<Duration>,
    /// If we should reset the range on next iteration.
    reset_back_off: Arc<AtomicBool>,
    /// If we should pause the loop on next iteration.
    paused: Arc<AtomicBool>,
}

impl TriggerSender {
    /// Create a new channel for waking a consumer
    pub fn new() -> (TriggerSender, TriggerReceiver) {
        let (tx, rx) = broadcast::channel(1);
        (
            TriggerSender {
                trigger: tx,
                reset_back_off: None,
                pause_back_off: None,
            },
            TriggerReceiver {
                rx,
                back_off: None,
                reset_on_trigger: false,
            },
        )
    }

    /// Create a new channel trigger that will also trigger
    /// on a loop.
    /// The duration takes a range so that the loop  can
    /// be set to back off from the lowest to the highest duration.
    /// If you do not want a back off, set the duration range
    /// to the same value like: `Duration::from_millis(10)..Duration::from_millis(10)`
    /// If reset_on_trigger is true, the back off will be reset whenever a
    /// trigger is received.
    pub fn new_with_loop(
        range: Range<Duration>,
        reset_on_trigger: bool,
    ) -> (TriggerSender, TriggerReceiver) {
        let (tx, rx) = broadcast::channel(1);
        let reset_back_off = Arc::new(AtomicBool::new(false));
        let pause_back_off = Arc::new(AtomicBool::new(false));
        (
            TriggerSender {
                trigger: tx,
                reset_back_off: Some(reset_back_off.clone()),
                pause_back_off: Some(pause_back_off.clone()),
            },
            TriggerReceiver {
                rx,
                reset_on_trigger,
                back_off: Some(BackOff::new(range, reset_back_off, pause_back_off)),
            },
        )
    }

    /// Lazily nudge the consumer task, ignoring the case where the consumer
    /// already has a pending trigger signal
    pub fn trigger(&self) {
        if self.trigger.send(()).is_err() {
            tracing::warn!(
                "Queue consumer trigger was sent while Cell is shutting down: ignoring."
            );
        };
    }

    /// Reset the back off to the lowest duration.
    /// If no back off is set this is a no-op.
    pub fn reset_back_off(&self) {
        if let Some(tx) = &self.reset_back_off {
            tx.store(true, Ordering::Relaxed);
        }
    }

    /// Pause the trigger loop if there is one.
    pub fn pause_loop(&self) {
        if let Some(pause) = &self.pause_back_off {
            pause.store(true, Ordering::Relaxed);
        }
    }

    /// Resume the trigger loop now if there is one.
    ///
    /// This will resume the loop even if it is currently
    /// listening (the workflow is not running).
    /// The downside to this call is that if the workflow
    /// is running it will immediately run a second time.
    ///
    /// This call is a no-op if the loop is not paused.
    pub fn resume_loop_now(&self) {
        if let Some(pause) = &self.pause_back_off {
            if pause.fetch_and(false, Ordering::AcqRel) {
                self.trigger();
            }
        }
    }

    /// Resume the trigger loop if there is one.
    ///
    /// This will cause the loop to to resume after the
    /// next trigger (or if the workflow is currently in progress).
    /// It will not cause the loop to resume immediately.
    /// If the loop is currently listening (the workflow is not running)
    /// then nothing will happen until the next trigger.
    /// See `resume_loop_now` for a version that will resume immediately.
    ///
    /// This call is a no-op if the loop is not paused.
    pub fn resume_loop(&self) {
        if let Some(pause) = &self.pause_back_off {
            pause.store(false, Ordering::Release);
        }
    }
}

impl TriggerReceiver {
    /// Listen for one or more items to come through, draining the channel
    /// each time. Bubble up errors on empty channel.
    pub async fn listen(&mut self) -> Result<(), QueueTriggerClosedError> {
        let Self {
            back_off,
            rx,
            reset_on_trigger,
        } = self;

        let mut was_trigger = true;
        {
            // Create the trigger future
            let trigger_fut = rx_fut(rx);
            match back_off {
                // We have a back off loop that is running.
                Some(back_off) if !back_off.is_paused() => {
                    let paused = back_off.paused.clone();
                    {
                        // Get the back off future.
                        let back_off_fut = back_off.wait();
                        futures::pin_mut!(back_off_fut, trigger_fut);

                        // Race between either a trigger or the loop.
                        match futures::future::select(trigger_fut, back_off_fut).await {
                            Either::Left((result, _)) => {
                                // We got a trigger, check the result and drop the wait future.
                                result?;
                            }
                            Either::Right((_, trigger_fut)) => {
                                // We got the loop future.
                                if paused.load(Ordering::Acquire) {
                                    // If we are now paused then we should wait for a trigger.
                                    trigger_fut.await?;
                                } else {
                                    // We are not pause so this was not a trigger.
                                    was_trigger = false;
                                }
                            }
                        }
                    }
                }
                _ => {
                    // We either have no back off loop or it's paused
                    // so wait for a trigger.
                    trigger_fut.await?;
                }
            }
        }
        // We want to flush the buffer if a trigger
        // that woke the listen.
        if was_trigger {
            // Do one try recv to empty the buffer.
            // This is needed as we can't have an empty buffer
            // but we don't want a second trigger to be stored in
            // the buffer and cause the workflow to run twice.
            let _ = self.rx.try_recv();

            // If we have a back off loop and got a trigger then
            // we should reset the back off if that flag is on.
            if *reset_on_trigger {
                if let Some(back_off) = back_off {
                    back_off.reset();
                }
            }
        }
        Ok(())
    }
}

/// Create a future that will be ok with either a recv or a lagged.
async fn rx_fut(rx: &mut broadcast::Receiver<()>) -> Result<(), QueueTriggerClosedError> {
    match rx.recv().await {
        Ok(_) => Ok(()),
        Err(broadcast::error::RecvError::Closed) => Err(QueueTriggerClosedError),
        Err(broadcast::error::RecvError::Lagged(_)) => Ok(()),
    }
}

impl BackOff {
    fn new(
        range: Range<Duration>,
        reset_back_off: Arc<AtomicBool>,
        pause_back_off: Arc<AtomicBool>,
    ) -> Self {
        Self {
            start: range.start,
            range,
            reset_back_off,
            paused: pause_back_off,
        }
    }

    async fn wait(&mut self) {
        // Check if we should reset the back off.
        if self.reset_back_off.fetch_and(false, Ordering::Relaxed) {
            self.reset();
        }
        // If the range is empty we are just looping.
        let dur = if self.range.is_empty() {
            self.range.end
        } else {
            // If not we take the current start value.
            self.range.start
        };
        // Sleep this task for the chosen duration.
        // This future may be cancelled during this await,
        // and any code following will not be executed.
        tokio::time::sleep(dur).await;
        // If the sleep completes then we bump the start of the range
        // or take the end if we have reached the end.
        self.range.start = std::cmp::min(self.range.start * 2, self.range.end);
    }

    fn reset(&mut self) {
        self.range.start = self.start;
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
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
fn handle_workflow_error(err: WorkflowError) -> ManagedTaskResult {
    Err(Box::new(ConductorError::from(err)).into())
}
