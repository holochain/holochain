//! We want to have control over certain long running
//! tasks that we care about.
//! If a task that is added to the task manager ends
//! then a reaction can be set.
//! An example would be a websocket closes with an error
//! and you want to restart it.

mod error;
pub use error::*;

use crate::conductor::error::ConductorError;
use crate::core::workflow::error::WorkflowError;
use futures::stream::FuturesUnordered;
use holochain_types::prelude::*;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::*;

use super::{conductor::StopBroadcaster, ConductorHandle};

const CHANNEL_SIZE: usize = 1000;

/// For a task to be "managed" simply means that it will shut itself down
/// when it receives a message on the the "stop" channel passed in
pub(crate) type ManagedTaskHandle = JoinHandle<ManagedTaskResult>;
pub(crate) type TaskManagerRunHandle = JoinHandle<TaskManagerResult>;

/// A generic function to run when a task completes
pub type OnDeath = Box<dyn Fn(ManagedTaskResult) -> TaskOutcome + Send + Sync + 'static>;

/// The "kind" of a managed task determines how the Result from the task's
/// completion will be handled.
pub enum TaskKind {
    /// Log an error if there is one, but otherwise do nothing.
    Ignore,
    /// If the task returns an error, shut down the conductor.
    Unrecoverable,
    /// If the task returns an error, "freeze" the cell which caused the error,
    /// but continue running the rest of the conductor and other managed tasks.
    CellCritical(CellId),
    /// A generic callback for handling the result
    // TODO: B-01455: reevaluate whether this should be a callback
    Generic(OnDeath),
}

/// A message sent to the TaskManager, registering an ManagedTask of a given kind.
pub struct ManagedTaskAdd {
    handle: ManagedTaskHandle,
    kind: TaskKind,
    name: String,
}

impl ManagedTaskAdd {
    fn new(handle: ManagedTaskHandle, kind: TaskKind, name: &str) -> Self {
        ManagedTaskAdd {
            handle,
            kind,
            name: name.to_string(),
        }
    }

    /// You just want the task in the task manager but don't want
    /// to react to an error
    pub fn ignore(handle: ManagedTaskHandle, name: &str) -> Self {
        Self::new(handle, TaskKind::Ignore, name)
    }

    /// If this task fails, the entire conductor must be shut down
    pub fn unrecoverable(handle: ManagedTaskHandle, name: &str) -> Self {
        Self::new(handle, TaskKind::Unrecoverable, name)
    }

    /// If this task fails, only the Cell which it runs under must be stopped
    pub fn cell_critical(handle: ManagedTaskHandle, cell_id: CellId, name: &str) -> Self {
        Self::new(handle, TaskKind::CellCritical(cell_id), name)
    }

    /// Handle a task's completion with a generic callback
    pub fn generic(handle: ManagedTaskHandle, f: OnDeath) -> Self {
        Self::new(handle, TaskKind::Generic(f), "unnamed")
    }
}

impl Future for ManagedTaskAdd {
    type Output = TaskOutcome;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let p = std::pin::Pin::new(&mut self.handle);
        match JoinHandle::poll(p, cx) {
            Poll::Ready(r) => Poll::Ready(handle_completed_task(
                &self.kind,
                r.unwrap_or_else(|e| Err(e.into())),
                self.name.clone(),
            )),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl std::fmt::Debug for ManagedTaskAdd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedTaskAdd").finish()
    }
}

/// The outcome of a task that has finished.
pub enum TaskOutcome {
    /// Spawn a new managed task.
    NewTask(ManagedTaskAdd),
    /// Ignore the exit and do nothing.
    Ignore,
    /// Ignore the exit and do nothing.
    MinorError(ManagedTaskError, String),
    /// Close the conductor down because this is an unrecoverable error.
    ShutdownConductor(Box<ManagedTaskError>, String),
    /// Remove the App which caused the panic, but let all other apps remain.
    UninstallApp(CellId, Box<ManagedTaskError>, String),
    /// Deactivate all apps which contain the problematic Cell.
    DeactivateApps(CellId, Box<ManagedTaskError>, String),
}

struct TaskManager {
    stream: FuturesUnordered<ManagedTaskAdd>,
}

impl TaskManager {
    fn new() -> Self {
        let stream = FuturesUnordered::new();
        TaskManager { stream }
    }
}

pub(crate) fn spawn_task_manager(
    handle: ConductorHandle,
) -> (mpsc::Sender<ManagedTaskAdd>, TaskManagerRunHandle) {
    let (send, recv) = mpsc::channel(CHANNEL_SIZE);
    (send, tokio::spawn(run(handle, recv)))
}

/// A super pessimistic task that is just waiting to die
/// but gets to live as long as the process
/// so the task manager doesn't quit
pub(crate) async fn keep_alive_task(mut die: broadcast::Receiver<()>) -> ManagedTaskResult {
    die.recv().await?;
    Ok(())
}

async fn run(
    conductor: ConductorHandle,
    mut new_task_channel: mpsc::Receiver<ManagedTaskAdd>,
) -> TaskManagerResult {
    let mut task_manager = TaskManager::new();
    // Need to have at least one item in the stream or it will exit early
    if let Some(new_task) = new_task_channel.recv().await {
        task_manager.stream.push(new_task);
    } else {
        error!("All senders to task manager were dropped before starting");
        return Err(TaskManagerError::TaskManagerFailedToStart);
    }
    loop {
        tokio::select! {
            Some(new_task) = new_task_channel.recv() => {
                task_manager.stream.push(new_task);
            }
            result = task_manager.stream.next() => match result {
                Some(TaskOutcome::NewTask(new_task)) => task_manager.stream.push(new_task),
                Some(TaskOutcome::Ignore) => (),
                Some(TaskOutcome::MinorError(error, context)) => {
                    error!("Minor error during managed task: {:?}\nContext: {}", error, context)
                }
                Some(TaskOutcome::ShutdownConductor(error, context)) => {
                    let error = match *error {
                        ManagedTaskError::Join(error) => {
                            match error.try_into_panic() {
                                Ok(reason) => {
                                    // Resume the panic on the main task
                                    std::panic::resume_unwind(reason);
                                }
                                Err(error) => ManagedTaskError::Join(error),
                            }
                        }
                        error => error,
                    };
                    error!("Shutting down conductor due to unrecoverable error: {:?}\nContext: {}", error, context);
                    return Err(TaskManagerError::Unrecoverable(error));
                },
                Some(TaskOutcome::UninstallApp(cell_id, error, context)) => {
                    tracing::error!("About to uninstall apps");
                    let app_ids = conductor.list_active_apps_for_cell_id(&cell_id).await.map_err(TaskManagerError::internal)?;
                    tracing::error!(
                        "UNINSTALLING the following apps due to an unrecoverable error during genesis: {:?}\nError: {:?}\nContext: {}",
                        app_ids,
                        error,
                        context
                    );
                    for app_id in app_ids.iter() {
                        conductor.uninstall_app(app_id).await.map_err(TaskManagerError::internal)?;
                    }
                    tracing::error!("Apps uninstalled.");
                },
                Some(TaskOutcome::DeactivateApps(cell_id, error, context)) => {
                    tracing::error!("About to deactivate apps");
                    let app_ids = conductor.list_active_apps_for_cell_id(&cell_id).await.map_err(TaskManagerError::internal)?;
                    tracing::error!(
                        "DEACTIVATING the following apps due to an unrecoverable error: {:?}\nError: {:?}\nContext: {}",
                        app_ids,
                        error,
                        context
                    );
                    for app_id in app_ids.iter() {
                        conductor.deactivate_app(app_id.to_string(), DeactivationReason::Quarantined { error: error.to_string() } ).await.map_err(TaskManagerError::internal)?;
                    }
                    tracing::error!("Apps quarantined via deactivation.");
                },
                None => return Ok(()),
            }
        };
    }
}

#[tracing::instrument(skip(kind))]
fn handle_completed_task(kind: &TaskKind, result: ManagedTaskResult, name: String) -> TaskOutcome {
    use TaskOutcome::*;
    match kind {
        TaskKind::Ignore => match result {
            Ok(_) => Ignore,
            Err(err) => MinorError(err, name),
        },
        TaskKind::Unrecoverable => match result {
            Ok(_) => Ignore,
            Err(err) => ShutdownConductor(Box::new(err), name),
        },
        TaskKind::CellCritical(cell_id) => match result {
            Ok(_) => Ignore,
            Err(err) => match &err {
                ManagedTaskError::Conductor(conductor_err) => match conductor_err {
                    // If the error was due to validation failure during genesis,
                    // just uninstall the app.
                    ConductorError::WorkflowError(
                        WorkflowError::AuthoredGenesisValidationRejection(_),
                    ) => UninstallApp(cell_id.to_owned(), Box::new(err), name),

                    // For all other errors, shut down the conductor
                    _ => ShutdownConductor(Box::new(err), name),
                },
                // If the task panicked, deactivate the app.
                // TODO: ideally, we could differentiate between the case of
                //   pre- and post-genesis failure, using UninstallApp for
                //   the former and DeactivateApps for the latter. However,
                //   there is no easy way to do this, so we simply deactivate
                //   in both cases, so we don't lose data.
                //   I think B-04188 would make this distinction possible.
                ManagedTaskError::Join(_) => {
                    DeactivateApps(cell_id.to_owned(), Box::new(err), name)
                }

                // For all others, shut down conductor
                _ => ShutdownConductor(Box::new(err), name),
            },
        },
        TaskKind::Generic(f) => f(result),
    }
}

/// Handle the result of shutting down the main thread.
pub fn handle_shutdown(result: Result<TaskManagerResult, tokio::task::JoinError>) {
    let result = result.map_err(|e| {
        error!(
            error = &e as &dyn std::error::Error,
            "Failed to join the main task"
        );
        e
    });
    match result {
        Ok(result) => result.expect("Conductor shutdown error"),
        Err(error) => match error.try_into_panic() {
            Ok(reason) => {
                // Resume the panic on the main task
                std::panic::resume_unwind(reason);
            }
            Err(error) => panic!("Error while joining threads during shutdown {:?}", error),
        },
    }
}

/// A collection of channels and handles used by the Conductor to talk to the
/// TaskManager task
pub struct TaskManagerClient {
    /// Channel on which to send info about tasks we want to manage
    task_add_sender: mpsc::Sender<ManagedTaskAdd>,

    /// Sending a message on this channel will broadcast to all managed tasks,
    /// telling them to shut down
    task_stop_broadcaster: StopBroadcaster,

    /// The main task join handle to await on.
    /// The conductor is intended to live as long as this task does.
    /// It can be moved out, hence the Option. If this is None, then the
    /// handle was already moved out.
    run_handle: Option<TaskManagerRunHandle>,
}

impl TaskManagerClient {
    /// Constructor
    pub fn new(
        task_add_sender: mpsc::Sender<ManagedTaskAdd>,
        task_stop_broadcaster: StopBroadcaster,
        run_handle: TaskManagerRunHandle,
    ) -> Self {
        Self {
            task_add_sender,
            task_stop_broadcaster,
            run_handle: Some(run_handle),
        }
    }

    /// Accessor
    pub fn task_add_sender(&self) -> &mpsc::Sender<ManagedTaskAdd> {
        &self.task_add_sender
    }

    /// Accessor
    pub fn task_stop_broadcaster(&self) -> &StopBroadcaster {
        &self.task_stop_broadcaster
    }

    /// Return the handle to be joined.
    /// This will return None if the handle was already taken.
    pub fn take_handle(&mut self) -> Option<TaskManagerRunHandle> {
        self.run_handle.take()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::error::ConductorError;
    use crate::conductor::handle::MockConductorHandleT;
    use anyhow::Result;
    use observability;
    use std::sync::Arc;

    #[tokio::test]
    async fn spawn_and_handle_dying_task() -> Result<()> {
        observability::test_run().ok();
        let mock_handle = MockConductorHandleT::new();
        let (send_task_handle, main_task) = spawn_task_manager(Arc::new(mock_handle));
        let handle = tokio::spawn(async {
            Err(ConductorError::Todo("This task gotta die".to_string()).into())
        });
        let handle = ManagedTaskAdd::generic(
            handle,
            Box::new(|result| match result {
                Ok(_) => panic!("Task should have died"),
                Err(ManagedTaskError::Conductor(ConductorError::Todo(_))) => {
                    let handle = tokio::spawn(async { Ok(()) });
                    let handle = ManagedTaskAdd::ignore(handle, "respawned task");
                    TaskOutcome::NewTask(handle)
                }
                _ => TaskOutcome::Ignore,
            }),
        );
        // Check that the main task doesn't close straight away
        let main_handle = tokio::spawn(main_task);
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Now send the handle
        if let Err(_) = send_task_handle.send(handle).await {
            panic!("Failed to send the handle");
        }
        main_handle.await???;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic]
    async fn unrecoverable_error() {
        observability::test_run().ok();
        let (_tx, rx) = tokio::sync::broadcast::channel(1);
        let mock_handle = MockConductorHandleT::new();
        let (send_task_handle, main_task) = spawn_task_manager(Arc::new(mock_handle));
        send_task_handle
            .send(ManagedTaskAdd::ignore(
                tokio::spawn(keep_alive_task(rx)),
                "",
            ))
            .await
            .unwrap();

        send_task_handle
            .send(ManagedTaskAdd::unrecoverable(
                tokio::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    Err(ConductorError::Todo("Unrecoverable task failed".to_string()).into())
                }),
                "",
            ))
            .await
            .unwrap();

        handle_shutdown(main_task.await);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic]
    async fn unrecoverable_panic() {
        observability::test_run().ok();
        let (_tx, rx) = tokio::sync::broadcast::channel(1);
        let mock_handle = MockConductorHandleT::new();
        let (send_task_handle, main_task) = spawn_task_manager(Arc::new(mock_handle));
        send_task_handle
            .send(ManagedTaskAdd::ignore(
                tokio::spawn(keep_alive_task(rx)),
                "",
            ))
            .await
            .unwrap();

        send_task_handle
            .send(ManagedTaskAdd::unrecoverable(
                tokio::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    panic!("Task has panicked")
                }),
                "",
            ))
            .await
            .unwrap();

        handle_shutdown(main_task.await);
    }
}
