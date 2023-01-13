//! We want to have control over certain long running
//! tasks that we care about.
//! If a task that is added to the task manager ends
//! then a reaction can be set.
//! An example would be a websocket closes with an error
//! and you want to restart it.

mod error;
pub use error::*;

use futures::Future;
use futures::FutureExt;
use holochain_types::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;
use task_motel::StopListener;
use tokio::task::JoinError;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::*;

use super::ConductorHandle;

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

/// Spawn a task which performs some action after each task has completed,
/// as recieved by the outcome channel produced by the task manager.
pub fn spawn_task_outcome_handler(
    conductor: ConductorHandle,
    mut outcomes: OutcomeReceiver,
) -> JoinHandle<TaskManagerResult> {
    tokio::spawn(async move {
        while let Some((_group, result)) = outcomes.next().await {
            match result {
                TaskOutcome::Noop => (),
                TaskOutcome::LogInfo(context) => {
                    debug!("Managed task completed: {}", context)
                }
                TaskOutcome::MinorError(error, context) => {
                    error!(
                        "Minor error during managed task: {:?}\nContext: {}",
                        error, context
                    )
                }
                TaskOutcome::ShutdownConductor(error, context) => {
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
                    error!(
                        "Shutting down conductor due to unrecoverable error: {:?}\nContext: {}",
                        error, context
                    );
                    return Err(TaskManagerError::Unrecoverable(Box::new(error)));
                }
                TaskOutcome::StopApps(cell_id, error, context) => {
                    tracing::error!("About to automatically stop apps");
                    let app_ids = conductor
                        .list_running_apps_for_dependent_cell_id(&cell_id)
                        .await
                        .map_err(TaskManagerError::internal)?;
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
                        let delta = conductor
                            .reconcile_app_status_with_cell_status(None)
                            .await
                            .map_err(TaskManagerError::internal)?;
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
                            conductor
                                .clone()
                                .disable_app(
                                    app_id.to_string(),
                                    DisabledAppReason::Error(error.to_string()),
                                )
                                .await
                                .map_err(TaskManagerError::internal)?;
                        }
                        tracing::error!("Apps disabled.");
                    }
                }
                TaskOutcome::StopAppsWithDna(dna_hash, error, context) => {
                    tracing::error!("About to automatically stop apps with dna {}", dna_hash);
                    let app_ids = conductor
                        .list_running_apps_for_dependent_dna_hash(dna_hash.as_ref())
                        .await
                        .map_err(TaskManagerError::internal)?;
                    if error.is_recoverable() {
                        let cells_with_same_dna: Vec<_> = conductor
                            .list_cell_ids(None)
                            .into_iter()
                            .filter(|id| id.dna_hash() == dna_hash.as_ref())
                            .collect();
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
                        let delta = conductor
                            .reconcile_app_status_with_cell_status(None)
                            .await
                            .map_err(TaskManagerError::internal)?;
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
                            conductor
                                .clone()
                                .disable_app(
                                    app_id.to_string(),
                                    DisabledAppReason::Error(error.to_string()),
                                )
                                .await
                                .map_err(TaskManagerError::internal)?;
                        }
                        tracing::error!("Apps disabled.");
                    }
                }
            };
        }
        Ok(())
    })
}

#[tracing::instrument(skip(kind))]
fn produce_task_outcome(
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

/// Each task has a group, and here they are
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskGroup {
    /// Tasks which are associated with the conductor as a whole
    Conductor,
    /// Tasks which are associated with a particular DNA space
    Dna(Arc<DnaHash>),
    /// Tasks which are associated with a particular running Cell
    Cell(CellId),
}

/// Channel receiver for task outcomes
pub type OutcomeReceiver = futures::channel::mpsc::Receiver<(TaskGroup, TaskOutcome)>;

/// A collection of channels and handles used by the Conductor to talk to the
/// TaskManager task
#[derive(Clone)]
pub struct TaskManagerClient {
    tm: Arc<Mutex<Option<task_motel::TaskManager<TaskGroup, TaskOutcome>>>>,
}

impl TaskManagerClient {
    /// Construct the TaskManager and the outcome channel receiver
    pub fn new() -> (Self, OutcomeReceiver) {
        let (tx, outcomes) = futures::channel::mpsc::channel(8);
        let tm = task_motel::TaskManager::new(tx, |g| match g {
            TaskGroup::Conductor => None,
            TaskGroup::Dna(_) => Some(TaskGroup::Conductor),
            TaskGroup::Cell(cell_id) => Some(TaskGroup::Dna(Arc::new(cell_id.dna_hash().clone()))),
        });
        let tm = Self {
            tm: Arc::new(Mutex::new(Some(tm))),
        };
        (tm, outcomes)
    }

    /// Stop all tasks and await their completion.
    pub fn stop_all_tasks(&self) -> ShutdownHandle {
        if let Some(tm) = self.tm.lock().as_mut() {
            tokio::spawn(tm.stop_group(&TaskGroup::Conductor))
        } else {
            tracing::warn!("Tried to shutdown task manager while it's already shutting down");
            tokio::spawn(async move {})
        }
    }

    /// Stop all tasks and return a future to await their completion,
    /// and prevent any new tasks from being added to the manager.
    pub fn shutdown(&mut self) -> ShutdownHandle {
        if let Some(mut tm) = self.tm.lock().take() {
            tokio::spawn(tm.stop_group(&TaskGroup::Conductor))
        } else {
            // already shutting down
            tokio::spawn(async move {})
        }
    }

    /// Add a conductor-level task whose outcome is ignored.
    pub fn add_conductor_task_ignored(
        &self,
        name: &str,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send + 'static,
    ) {
        self.add_conductor_task(name, TaskKind::Ignore, f)
    }

    /// Add a conductor-level task which will cause the conductor to shut down if it fails
    pub fn add_conductor_task_unrecoverable(
        &self,
        name: &str,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send + 'static,
    ) {
        self.add_conductor_task(name, TaskKind::Unrecoverable, f)
    }

    /// Add a DNA-level task which will cause all cells under that DNA to be disabled if
    /// the task fails
    pub fn add_dna_task_critical(
        &self,
        name: &str,
        dna_hash: Arc<DnaHash>,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send + 'static,
    ) {
        self.add_dna_task(name, TaskKind::DnaCritical(dna_hash.clone()), dna_hash, f)
    }

    /// Add a Cell-level task whose outcome is ignored
    pub fn add_cell_task_ignored(
        &self,
        name: &str,
        cell_id: CellId,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send + 'static,
    ) {
        self.add_cell_task(name, TaskKind::Ignore, cell_id, f)
    }

    /// Add a Cell-level task which will cause that to be disabled if the task fails
    pub fn add_cell_task_critical(
        &self,
        name: &str,
        cell_id: CellId,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send + 'static,
    ) {
        self.add_cell_task(name, TaskKind::CellCritical(cell_id.clone()), cell_id, f)
    }

    fn add_conductor_task(
        &self,
        name: &str,
        task_kind: TaskKind,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send + 'static,
    ) {
        let name = name.to_string();
        let f = move |stop| f(stop).map(move |t| produce_task_outcome(&task_kind, t, name));
        self.add_task(TaskGroup::Conductor, f)
    }

    fn add_dna_task(
        &self,
        name: &str,
        task_kind: TaskKind,
        dna_hash: Arc<DnaHash>,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send + 'static,
    ) {
        let name = name.to_string();
        let f = move |stop| f(stop).map(move |t| produce_task_outcome(&task_kind, t, name));
        self.add_task(TaskGroup::Dna(dna_hash), f)
    }

    fn add_cell_task(
        &self,
        name: &str,
        task_kind: TaskKind,
        cell_id: CellId,
        f: impl FnOnce(StopListener) -> JoinHandle<ManagedTaskResult> + Send + 'static,
    ) {
        let name = name.to_string();
        let f = move |stop| f(stop).map(move |t| produce_task_outcome(&task_kind, t, name));
        self.add_task(TaskGroup::Cell(cell_id), f)
    }

    fn add_task<Fut: Future<Output = TaskOutcome> + Send + 'static>(
        &self,
        group: TaskGroup,
        f: impl FnOnce(StopListener) -> Fut + Send + 'static,
    ) {
        dbg!();
        if let Some(tm) = self.tm.lock().as_mut() {
            dbg!();
            tm.add_task(group, f)
        } else {
            tracing::warn!("Tried to add task while task manager is shutting down.");
        }
    }
}

// impl Drop for TaskManagerClient {
//     fn drop(&mut self) {
//         self.shutdown();
//     }
// }

/// A future which awaits the completion of all managed tasks
pub type ShutdownHandle = JoinHandle<()>;

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::{error::ConductorError, Conductor};
    use holochain_state::test_utils::test_db_dir;
    use observability;

    #[tokio::test(flavor = "multi_thread")]
    async fn unrecoverable_error() {
        observability::test_run().ok();
        let db_dir = test_db_dir();
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();
        let tm = handle.task_manager();
        tm.add_conductor_task_unrecoverable("unrecoverable", |_stop| {
            dbg!();
            tokio::spawn(async {
                dbg!();
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                dbg!();
                Err(Box::new(ConductorError::Other(
                    anyhow::anyhow!("Unrecoverable task failed").into(),
                ))
                .into())
            })
        });

        let (tm, main_task) = handle.task_manager.share_mut(|o| o.take().unwrap());

        // the outcome channel sender lives on the TaskManager, so we need to drop it
        // so that the main_task will end

        // tm.shutdown();
        drop(tm);

        main_task
            .await
            .expect("Failed to join the main task")
            .expect_err("The main task should return an error");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "panics in tokio break other tests, this test is here to confirm behavior but cannot be run on ci"]
    async fn unrecoverable_panic() {
        observability::test_run().ok();
        let db_dir = test_db_dir();
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();
        let tm = handle.task_manager();

        tm.add_conductor_task_unrecoverable("unrecoverable", |_stop| {
            tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                panic!("Task has panicked")
            })
        });

        let (_, main_task) = handle.task_manager.share_mut(|o| o.take().unwrap());
        drop(tm);
        handle_shutdown(main_task.await);
    }
}
