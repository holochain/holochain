//! We want to have control over certain long running
//! tasks that we care about.
//! If a task that is added to the task manager ends
//! then a reaction can be set.
//! An example would be a websocket closes with an error
//! and you want to restart it.

mod error;
pub use error::*;

use futures::stream::FuturesUnordered;
use futures::FutureExt;
use holochain_types::prelude::*;
use parking_lot::Mutex;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use task_motel::StopListener;
use task_motel::Task;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinError;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::*;

use super::{conductor::StopBroadcaster, ConductorHandle};

const CHANNEL_SIZE: usize = 1000;

/// For a task to be "managed" simply means that it will shut itself down
/// when it receives a message on the the "stop" channel passed in
pub(crate) type ManagedTaskHandle = JoinHandle<ManagedTaskResult>;
pub(crate) type TaskManagerRunHandle = JoinHandle<TaskManagerResult>;

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
    /// If the task returns an error, "freeze" all cells with this dna hash,
    /// but continue running the rest of the conductor and other managed tasks.
    DnaCritical(Arc<DnaHash>),
}

/// The outcome of a task that has finished.
pub enum TaskOutcome {
    /// Do nothing
    Noop,
    /// Log an info trace and take no other action.
    LogInfo(String),
    /// Log an error and take no other action.
    MinorError(Box<ManagedTaskError>, String),
    /// Close the conductor down because this is an unrecoverable error.
    ShutdownConductor(Box<ManagedTaskError>, String),
    /// Either pause or disable all apps which contain the problematic Cell,
    /// depending upon the specific error.
    StopApps(CellId, Box<ManagedTaskError>, String),
    /// Either pause or disable all apps which contain the problematic Dna,
    /// depending upon the specific error.
    StopAppsWithDna(Arc<DnaHash>, Box<ManagedTaskError>, String),
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
                tracing::debug!("Task added. Total tasks: {}", task_manager.stream.len());
            }
            result = task_manager.stream.next() => {
                tracing::debug!("Task completed. Total tasks: {}", task_manager.stream.len());
                match result {
                    Some(TaskOutcome::Noop) => (),
                Some(TaskOutcome::LogInfo(context)) => {
                    debug!("Managed task completed: {}", context)
                }
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
                    return Err(TaskManagerError::Unrecoverable(Box::new(error)));
                },
                Some(TaskOutcome::StopApps(cell_id, error, context)) => {
                    tracing::error!("About to automatically stop apps");
                    let app_ids = conductor.list_running_apps_for_dependent_cell_id(&cell_id).await.map_err(TaskManagerError::internal)?;
                    if error.is_recoverable() {
                        conductor.remove_cells(&[cell_id]).await;

                        // The following message assumes that only the app_ids calculated will be paused, but other apps
                        // may have been paused as well.
                        tracing::error!(
                            "PAUSING the following apps due to a recoverable error: {:?}\nError: {:?}\nContext: {}",
                            app_ids,
                            error,
                            context
                        );

                        // MAYBE: it could be helpful to modify this function so that when providing Some(app_ids),
                        //   you can also pass in a PausedAppReason override, so that the reason for the apps being paused
                        //   can be set to the specific error message encountered here, rather than having to read it from
                        //   the logs.
                        let delta = conductor.reconcile_app_status_with_cell_status(None).await.map_err(TaskManagerError::internal)?;
                        tracing::debug!(delta = ?delta);

                        tracing::error!("Apps paused.");
                    } else {
                        // Since the error is unrecoverable, we don't expect to be able to use this Cell anymore.
                        // Therefore, we disable every app which requires that cell.
                        tracing::error!(
                            "DISABLING the following apps due to an unrecoverable error: {:?}\nError: {:?}\nContext: {}",
                            app_ids,
                            error,
                            context
                        );
                        for app_id in app_ids.iter() {
                            conductor.clone().disable_app(app_id.to_string(), DisabledAppReason::Error(error.to_string())).await.map_err(TaskManagerError::internal)?;
                        }
                        tracing::error!("Apps disabled.");
                    }
                },
                Some(TaskOutcome::StopAppsWithDna(dna_hash, error, context)) => {
                    tracing::error!("About to automatically stop apps with dna {}", dna_hash);
                    let app_ids = conductor.list_running_apps_for_dependent_dna_hash(dna_hash.as_ref()).await.map_err(TaskManagerError::internal)?;
                    if error.is_recoverable() {
                        let cells_with_same_dna: Vec<_> = conductor.list_cell_ids(None).into_iter().filter(|id| id.dna_hash() == dna_hash.as_ref()).collect();
                        conductor.remove_cells(&cells_with_same_dna).await;

                        // The following message assumes that only the app_ids calculated will be paused, but other apps
                        // may have been paused as well.
                        tracing::error!(
                            "PAUSING the following apps due to a recoverable error: {:?}\nError: {:?}\nContext: {}",
                            app_ids,
                            error,
                            context
                        );

                        // MAYBE: it could be helpful to modify this function so that when providing Some(app_ids),
                        //   you can also pass in a PausedAppReason override, so that the reason for the apps being paused
                        //   can be set to the specific error message encountered here, rather than having to read it from
                        //   the logs.
                        let delta = conductor.reconcile_app_status_with_cell_status(None).await.map_err(TaskManagerError::internal)?;
                        tracing::debug!(delta = ?delta);

                        tracing::error!("Apps paused.");
                    } else {
                        // Since the error is unrecoverable, we don't expect to be able to use this Cell anymore.
                        // Therefore, we disable every app which requires that cell.
                        tracing::error!(
                            "DISABLING the following apps due to an unrecoverable error: {:?}\nError: {:?}\nContext: {}",
                            app_ids,
                            error,
                            context
                        );
                        for app_id in app_ids.iter() {
                            conductor.clone().disable_app(app_id.to_string(), DisabledAppReason::Error(error.to_string())).await.map_err(TaskManagerError::internal)?;
                        }
                        tracing::error!("Apps disabled.");
                    }
                },
                None => return Ok(()),
            }}
        };
    }
}

#[tracing::instrument(skip(kind))]
fn handle_completed_task(
    kind: &TaskKind,
    result: Result<ManagedTaskResult, JoinError>,
    name: String,
) -> TaskOutcome {
    use TaskOutcome::*;
    match kind {
        TaskKind::Ignore => match result {
            Err(err) => LogInfo(format!("task completed: {}, join error: {:?}", name, err)),
            Ok(Ok(_)) => LogInfo(format!("task completed: {}", name)),
            Ok(Err(err)) => MinorError(Box::new(err), name),
        },
        TaskKind::Unrecoverable => match result {
            Err(err) => LogInfo(format!("task completed: {}, join error: {:?}", name, err)),
            Ok(Ok(_)) => LogInfo(format!("task completed: {}", name)),
            Ok(Err(err)) => ShutdownConductor(Box::new(err), name),
        },
        TaskKind::CellCritical(cell_id) => match result {
            Err(err) => LogInfo(format!("task completed: {}, join error: {:?}", name, err)),
            Ok(Ok(_)) => LogInfo(format!("task completed: {}", name)),
            Ok(Err(err)) => StopApps(cell_id.to_owned(), Box::new(err), name),
        },
        TaskKind::DnaCritical(dna_hash) => match result {
            Err(err) => LogInfo(format!("task completed: {}, join error: {:?}", name, err)),
            Ok(Ok(_)) => LogInfo(format!("task completed: {}", name)),
            Ok(Err(err)) => StopAppsWithDna(dna_hash.to_owned(), Box::new(err), name),
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

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum TaskGroup {
    Conductor,
    Cell(CellId),
}

pub type OutcomeReceiver = futures::channel::mpsc::Receiver<(TaskGroup, Outcome)>;
/// A collection of channels and handles used by the Conductor to talk to the
/// TaskManager task
#[derive(Clone)]
pub struct TaskManagerClient {
    tm: Arc<Mutex<task_motel::TaskManager<TaskGroup, TaskOutcome>>>,
}

impl TaskManagerClient {
    pub fn new() -> (Self, OutcomeReceiver) {
        let (tx, outcomes) = futures::channel::mpsc::channel(8);
        let tm = Self {
            tm: Arc::new(Mutex::new(task_motel::TaskManager::new(tx, |g| match g {
                TaskGroup::Conductor => None,
                TaskGroup::Cell(_) => Some(TaskGroup::Conductor),
            }))),
        };
        (tm, outcomes)
    }

    pub fn add_conductor_task(
        &self,
        name: &str,
        task_kind: TaskKind,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send,
    ) {
        let f = move |stop| f(stop).map(|t| handle_completed_task(&task_kind, t, name.into()));
        self.tm.lock().add_task(TaskGroup::Conductor, f)
    }

    pub fn add_cell_task(
        &self,
        name: &str,
        task_kind: TaskKind,
        cell_id: CellId,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send,
    ) {
        let f = |stop| f(stop).map(|t| handle_completed_task(&task_kind, t, name.into()));
        self.tm.lock().add_task(TaskGroup::Cell(cell_id), f)
    }

    pub fn shutdown(&self) -> ShutdownHandle {
        self.tm.lock().stop_group(&TaskGroup::Conductor)
    }
}

pub type ShutdownHandle = task_motel::GroupStop;

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::{error::ConductorError, Conductor};
    use anyhow::Result;
    use holochain_state::test_utils::test_db_dir;
    use observability;

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_and_handle_dying_task() -> Result<()> {
        observability::test_run().ok();
        let db_dir = test_db_dir();
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();
        let (send_task_handle, main_task) = spawn_task_manager(handle);
        let handle = tokio::spawn(async {
            Err(Box::new(ConductorError::Other(
                anyhow::anyhow!("This task gotta die").into(),
            ))
            .into())
        });
        let handle = ManagedTaskAdd::generic(
            handle,
            Box::new(|result| match result {
                Ok(_) => panic!("Task should have died"),
                Err(ManagedTaskError::Conductor(err))
                    if matches!(*err, ConductorError::Other(_)) =>
                {
                    let handle = tokio::spawn(async { Ok(()) });
                    let handle = ManagedTaskAdd::ignore(handle, "respawned task");
                    TaskOutcome::NewTask(handle)
                }
                Err(_) => unreachable!("No other error is created by this test."),
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
    async fn unrecoverable_error() {
        observability::test_run().ok();
        let (_tx, rx) = tokio::sync::broadcast::channel(1);
        let db_dir = test_db_dir();
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();
        let (send_task_handle, main_task) = spawn_task_manager(handle);
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
                    Err(Box::new(ConductorError::Other(
                        anyhow::anyhow!("Unrecoverable task failed").into(),
                    ))
                    .into())
                }),
                "",
            ))
            .await
            .unwrap();

        main_task
            .await
            .expect("Failed to join the main task")
            .expect_err("The main task should return an error");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "panics in tokio break other tests, this test is here to confirm behavior but cannot be run on ci"]
    async fn unrecoverable_panic() {
        observability::test_run().ok();
        let (_tx, rx) = tokio::sync::broadcast::channel(1);
        let db_dir = test_db_dir();
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();
        let (send_task_handle, main_task) = spawn_task_manager(handle);
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
