//! Holochain Task Manager
//!
//! The TaskManager is used to manage long running tasks that are critical to the
//! operation of the conductor.
//!
//! Tasks added to the manager can be in one of three groups:
//!
//! - Conductor: Tasks which are associated with the conductor as a whole
//! - Dna: Tasks which are associated with a particular DNA
//! - Cell: Tasks which are associated with a particular Cell
//!
//! The outcome of a task in a group can affect the other tasks in its group.
//! Tasks which are critical to the operation of its group level will cause
//! the other tasks in that group to be stopped.
//!
//! For instance, the tasks which run the workflows for a cell are critical
//! to the cell's functioning, so if any of these tasks fail, then the cell
//! is no longer able to function. Task failure is a signal that the cell
//! needs to be shut down, so the task manager takes the steps necessary to
//! accomplish that:
//!
//! 1. Stop all other tasks related to the cell, so they don't continue in the background.
//! 2. Disable any apps which depend on the cell, because the app cannot
//!    function without the proper functioning of that cell.

mod error;
pub use error::*;

use futures::Future;
use futures::FutureExt;
use holochain_types::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;
use task_motel::StopListener;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::*;

use super::ConductorHandle;

/// The main interface for interacting with a task manager.
/// Contains functions for adding tasks to groups, stopping task groups,
/// and shutting down the task manager.
#[derive(Clone)]
pub struct TaskManagerClient {
    tm: Arc<Mutex<Option<task_motel::TaskManager<TaskGroup, TaskOutcome>>>>,
}

impl TaskManagerClient {
    /// Construct the TaskManager and the outcome channel receiver
    pub fn new(tx: OutcomeSender, scope: String) -> Self {
        let span = tracing::info_span!("managed task", scope = scope);
        let tm = task_motel::TaskManager::new_instrumented(span, tx, |g| match g {
            TaskGroup::Conductor => None,
            TaskGroup::Dna(_) => Some(TaskGroup::Conductor),
            TaskGroup::Cell(cell_id) => Some(TaskGroup::Dna(Arc::new(cell_id.dna_hash().clone()))),
        });
        Self {
            tm: Arc::new(Mutex::new(Some(tm))),
        }
    }

    /// Stop all managed tasks and await their completion.
    pub fn stop_all_tasks(&self) -> ShutdownHandle {
        if let Some(tm) = self.tm.lock().as_mut() {
            tokio::spawn(tm.stop_group(&TaskGroup::Conductor))
        } else {
            tracing::warn!("Tried to shutdown task manager while it's already shutting down");
            tokio::spawn(async move {})
        }
    }

    /// Stop all tasks associated with a Cell and await their completion.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub fn stop_cell_tasks(&self, cell_id: CellId) -> ShutdownHandle {
        if let Some(tm) = self.tm.lock().as_mut() {
            tokio::spawn(tm.stop_group(&TaskGroup::Cell(cell_id)).in_current_span())
        } else {
            tracing::warn!("Tried to shutdown cell's tasks while they're already shutting down");
            tokio::spawn(async move {})
        }
    }

    /// Stop all tasks and prevent any new tasks from being added to the manager.
    /// Returns a future to await completion of all tasks.
    pub fn shutdown(&mut self) -> ShutdownHandle {
        if let Some(mut tm) = self.tm.lock().take() {
            tokio::spawn(tm.stop_group(&TaskGroup::Conductor))
        } else {
            // already shutting down
            tokio::spawn(async move {})
        }
    }

    /// Add a conductor-level task whose outcome is ignored.
    pub fn add_conductor_task_ignored<Fut: Future<Output = ManagedTaskResult> + Send + 'static>(
        &self,
        name: &str,
        f: impl FnOnce() -> Fut + Send + 'static,
    ) {
        self.add_conductor_task(name, TaskKind::Ignore, move |stop| async move {
            tokio::select! {
                _ = stop => (),
                _ = f() => (),
            }
            Ok(())
        })
    }

    /// Add a conductor-level task which will cause the conductor to shut down if it fails
    pub fn add_conductor_task_unrecoverable<
        Fut: Future<Output = ManagedTaskResult> + Send + 'static,
    >(
        &self,
        name: &str,
        f: impl FnOnce(StopListener) -> Fut + Send + 'static,
    ) {
        self.add_conductor_task(name, TaskKind::Unrecoverable, f)
    }

    /// Add a DNA-level task which will cause all cells under that DNA to be disabled if
    /// the task fails
    pub fn add_dna_task_critical<Fut: Future<Output = ManagedTaskResult> + Send + 'static>(
        &self,
        name: &str,
        dna_hash: Arc<DnaHash>,
        f: impl FnOnce(StopListener) -> Fut + Send + 'static,
    ) {
        self.add_dna_task(name, TaskKind::DnaCritical(dna_hash.clone()), dna_hash, f)
    }

    /// Add a Cell-level task whose outcome is ignored
    pub fn add_cell_task_ignored<Fut: Future<Output = ManagedTaskResult> + Send + 'static>(
        &self,
        name: &str,
        cell_id: CellId,
        f: impl FnOnce(StopListener) -> Fut + Send + 'static,
    ) {
        self.add_cell_task(name, TaskKind::Ignore, cell_id, f)
    }

    /// Add a Cell-level task which will cause that to be disabled if the task fails
    pub fn add_cell_task_critical<Fut: Future<Output = ManagedTaskResult> + Send + 'static>(
        &self,
        name: &str,
        cell_id: CellId,
        f: impl FnOnce(StopListener) -> Fut + Send + 'static,
    ) {
        self.add_cell_task(name, TaskKind::CellCritical(cell_id.clone()), cell_id, f)
    }

    fn add_conductor_task<Fut: Future<Output = ManagedTaskResult> + Send + 'static>(
        &self,
        name: &str,
        task_kind: TaskKind,
        f: impl FnOnce(StopListener) -> Fut + Send + 'static,
    ) {
        let name = name.to_string();
        let f = move |stop| f(stop).map(move |t| produce_task_outcome(&task_kind, t, name));
        self.add_task(TaskGroup::Conductor, f)
    }

    fn add_dna_task<Fut: Future<Output = ManagedTaskResult> + Send + 'static>(
        &self,
        name: &str,
        task_kind: TaskKind,
        dna_hash: Arc<DnaHash>,
        f: impl FnOnce(StopListener) -> Fut + Send + 'static,
    ) {
        let name = name.to_string();
        let f = move |stop| f(stop).map(move |t| produce_task_outcome(&task_kind, t, name));
        self.add_task(TaskGroup::Dna(dna_hash), f)
    }

    fn add_cell_task<Fut: Future<Output = ManagedTaskResult> + Send + 'static>(
        &self,
        name: &str,
        task_kind: TaskKind,
        cell_id: CellId,
        f: impl FnOnce(StopListener) -> Fut + Send + 'static,
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
        if let Some(tm) = self.tm.lock().as_mut() {
            tm.add_task(group, f)
        } else {
            tracing::warn!("Tried to add task while task manager is shutting down.");
        }
    }
}

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
    /// Log an info trace and take no other action.
    LogInfo(String),
    /// Log an error and take no other action.
    MinorError(Box<ManagedTaskError>, String),
    /// Close the conductor down because this is an unrecoverable error.
    ShutdownConductor(Box<ManagedTaskError>, String),
    /// Disable all apps which contain the problematic Cell,
    /// depending upon the specific error.
    StopApps(CellId, Box<ManagedTaskError>, String),
    /// Disable all apps which contain the problematic DNA,
    /// depending upon the specific error.
    StopAppsWithDna(Arc<DnaHash>, Box<ManagedTaskError>, String),
}

/// Spawn a task which performs some action after each task has completed,
/// as recieved by the outcome channel produced by the task manager.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub(crate) fn spawn_task_outcome_handler(
    conductor: ConductorHandle,
    mut outcomes: OutcomeReceiver,
) -> JoinHandle<TaskManagerResult> {
    let span = tracing::info_span!(
        "spawn_task_outcome_handler",
        scope = conductor.get_config().tracing_scope()
    );
    tokio::spawn(async move {
        while let Some((_group, result)) = outcomes.next().await {
            match result {
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
                        .list_enabled_apps_for_dependent_cell_id(&cell_id)
                        .await
                        .map_err(TaskManagerError::internal)?;
                    // Disable every app which requires that cell.
                    tracing::error!(
                        "DISABLING the following apps due to an error: {:?}\nError: {:?}\nContext: {}",
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
                TaskOutcome::StopAppsWithDna(dna_hash, error, context) => {
                    tracing::error!("About to automatically stop apps with dna {}", dna_hash);
                    let app_ids = conductor
                        .list_enabled_apps_for_dependent_dna_hash(dna_hash.as_ref())
                        .await
                        .map_err(TaskManagerError::internal)?;
                    // Disable every app which requires that cell.
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
            };
        }
        Ok(())
    }.instrument(span))
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip(kind)))]
fn produce_task_outcome(kind: &TaskKind, result: ManagedTaskResult, name: String) -> TaskOutcome {
    use TaskOutcome::*;
    match kind {
        TaskKind::Ignore => match result {
            Ok(_) => LogInfo(format!("task completed: {}", name)),
            Err(err) => MinorError(Box::new(err), name),
        },
        TaskKind::Unrecoverable => match result {
            Ok(_) => LogInfo(format!("task completed: {}", name)),
            Err(err) => ShutdownConductor(Box::new(err), name),
        },
        TaskKind::CellCritical(cell_id) => match result {
            Ok(_) => LogInfo(format!("task completed: {}", name)),
            Err(err) => StopApps(cell_id.to_owned(), Box::new(err), name),
        },
        TaskKind::DnaCritical(dna_hash) => match result {
            Ok(_) => LogInfo(format!("task completed: {}", name)),
            Err(err) => StopAppsWithDna(dna_hash.to_owned(), Box::new(err), name),
        },
    }
}

/// Handle the result of shutting down the main thread.
pub fn handle_shutdown(result: Result<TaskManagerResult, tokio::task::JoinError>) {
    let result = result.inspect_err(|e| {
        error!(
            error = e as &dyn std::error::Error,
            "Failed to join the main task"
        );
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

/// Channel sender for task outcomes
pub type OutcomeSender = futures::channel::mpsc::Sender<(TaskGroup, TaskOutcome)>;
/// Channel receiver for task outcomes
pub type OutcomeReceiver = futures::channel::mpsc::Receiver<(TaskGroup, TaskOutcome)>;

/// A future which awaits the completion of all managed tasks
pub type ShutdownHandle = JoinHandle<()>;

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::{error::ConductorError, Conductor};
    use holochain_state::test_utils::test_db_dir;
    use holochain_trace;

    #[tokio::test(flavor = "multi_thread")]
    async fn unrecoverable_error() {
        holochain_trace::test_run();
        let db_dir = test_db_dir();
        let handle = Conductor::builder()
            .with_data_root_path(db_dir.path().to_path_buf().into())
            .test(&[])
            .await
            .unwrap();
        let tm = handle.task_manager();
        tm.add_conductor_task_unrecoverable("unrecoverable", |_stop| async {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            Err(Box::new(ConductorError::Other(
                anyhow::anyhow!("Unrecoverable task failed").into(),
            ))
            .into())
        });

        let main_task = handle.outcomes_task.share_mut(|o| o.take().unwrap());

        // the outcome channel sender lives on the TaskManager, so we need to drop it
        // so that the main_task will end

        drop(tm);

        main_task
            .await
            .expect("Failed to join the main task")
            .expect_err("The main task should return an error");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "panics in tokio break other tests, this test is here to confirm behavior but cannot be run on ci"]
    async fn unrecoverable_panic() {
        holochain_trace::test_run();
        let db_dir = test_db_dir();
        let handle = Conductor::builder()
            .with_data_root_path(db_dir.as_ref().to_path_buf().into())
            .test(&[])
            .await
            .unwrap();
        let tm = handle.task_manager();

        tm.add_conductor_task_unrecoverable("unrecoverable", |_stop| async {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            panic!("Task has panicked")
        });

        let main_task = handle.outcomes_task.share_mut(|o| o.take().unwrap());
        drop(tm);
        handle_shutdown(main_task.await);
    }
}
