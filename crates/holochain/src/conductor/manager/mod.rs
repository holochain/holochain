//! We want to have control over certain long running
//! tasks that we care about.
//! If a task that is added to the task manager ends
//! then a reaction can be set.
//! An example would be a websocket closes with an error
//! and you want to restart it.

mod error;
pub use error::*;

use futures::stream::FuturesUnordered;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::*;

const CHANNEL_SIZE: usize = 1000;

/// For a task to be "managed" simply means that it will shut itself down
/// when it receives a message on the the "stop" channel passed in
pub(crate) type ManagedTaskHandle = JoinHandle<ManagedTaskResult>;
pub(crate) type TaskManagerRunHandle = JoinHandle<ShutdownResult>;

pub(crate) type OnDeath = Box<dyn Fn(ManagedTaskResult) -> TaskOutcome + Send + Sync>;

/// A message sent to the TaskManager, registering an OnDeath closure to run upon
/// completion of a task.
///
/// The closure may itself return a new ManagedTaskAdd, which will cause another task to be
/// added while this one is being removed.
pub struct ManagedTaskAdd {
    handle: ManagedTaskHandle,
    // TODO: B-01455: reevaluate whether this should be a callback
    on_death: OnDeath,
}

impl ManagedTaskAdd {
    pub(crate) fn new(handle: ManagedTaskHandle, on_death: OnDeath) -> Self {
        ManagedTaskAdd { handle, on_death }
    }

    /// You just want the task in the task manager but don't want
    /// to react to an error
    pub(crate) fn ignore(handle: ManagedTaskHandle) -> Self {
        let on_death = Box::new(|_| TaskOutcome::Ignore);
        Self::new(handle, on_death)
    }

    pub(crate) fn unrecoverable(handle: ManagedTaskHandle) -> Self {
        let on_death = Box::new(|r| {
            match r {
                // Normal shutdown.
                Ok(_) => TaskOutcome::Ignore,
                // Task failed.
                Err(e) => TaskOutcome::ExitConductor(Box::new(e)),
            }
        });
        Self::new(handle, on_death)
    }
}

impl Future for ManagedTaskAdd {
    type Output = TaskOutcome;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let p = std::pin::Pin::new(&mut self.handle);
        match JoinHandle::poll(p, cx) {
            Poll::Ready(r) => Poll::Ready(handle_completed_task(
                &self.on_death,
                r.unwrap_or_else(|e| Err(e.into())),
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
    /// Close the conductor down because this is an unrecoverable error.
    ExitConductor(Box<ManagedTaskError>),
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

pub(crate) fn spawn_task_manager() -> (mpsc::Sender<ManagedTaskAdd>, TaskManagerRunHandle) {
    let (send, recv) = mpsc::channel(CHANNEL_SIZE);
    (send, tokio::spawn(run(recv)))
}

/// A super pessimistic task that is just waiting to die
/// but gets to live as long as the process
/// so the task manager doesn't quit
pub(crate) async fn keep_alive_task(mut die: broadcast::Receiver<()>) -> ManagedTaskResult {
    die.recv().await?;
    Ok(())
}

async fn run(mut new_task_channel: mpsc::Receiver<ManagedTaskAdd>) -> ShutdownResult {
    let mut task_manager = TaskManager::new();
    // Need to have at least one item in the stream or it will exit early
    if let Some(new_task) = new_task_channel.recv().await {
        task_manager.stream.push(new_task);
    } else {
        error!("All senders to task manager were dropped before starting");
        return Err(ShutdownError::TaskManagerFailedToStart);
    }
    loop {
        tokio::select! {
            Some(new_task) = new_task_channel.recv() => {
                task_manager.stream.push(new_task);
            }
            result = task_manager.stream.next() => match result {
                Some(TaskOutcome::NewTask(new_task)) => task_manager.stream.push(new_task),
                Some(TaskOutcome::Ignore) => (),
                Some(TaskOutcome::ExitConductor(error)) => {
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
                    error!("Shutting down conductor due to unrecoverable error: {:?}", error);
                    return Err(ShutdownError::Unrecoverable(error));
                },
                None => return Ok(()),
            }
        };
    }
}

fn handle_completed_task(on_death: &OnDeath, task_result: ManagedTaskResult) -> TaskOutcome {
    on_death(task_result)
}

/// Handle the result of shutting down the main thread.
pub fn handle_shutdown(result: Result<ShutdownResult, tokio::task::JoinError>) {
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::error::ConductorError;
    use anyhow::Result;
    use observability;

    #[tokio::test]
    async fn spawn_and_handle_dying_task() -> Result<()> {
        observability::test_run().ok();
        let (send_task_handle, main_task) = spawn_task_manager();
        let handle = tokio::spawn(async {
            Err(ConductorError::Todo("This task gotta die".to_string()).into())
        });
        let handle = ManagedTaskAdd::new(
            handle,
            Box::new(|result| match result {
                Ok(_) => panic!("Task should have died"),
                Err(ManagedTaskError::Conductor(ConductorError::Todo(_))) => {
                    let handle = tokio::spawn(async { Ok(()) });
                    let handle = ManagedTaskAdd::new(handle, Box::new(|_| TaskOutcome::Ignore));
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
        let (send_task_handle, main_task) = spawn_task_manager();
        send_task_handle
            .send(ManagedTaskAdd::ignore(tokio::spawn(keep_alive_task(rx))))
            .await
            .unwrap();

        send_task_handle
            .send(ManagedTaskAdd::unrecoverable(tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                Err(ConductorError::Todo("Unrecoverable task failed".to_string()).into())
            })))
            .await
            .unwrap();
        
        handle_shutdown(main_task.await);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic]
    async fn unrecoverable_panic() {
        observability::test_run().ok();
        let (_tx, rx) = tokio::sync::broadcast::channel(1);
        let (send_task_handle, main_task) = spawn_task_manager();
        send_task_handle
            .send(ManagedTaskAdd::ignore(tokio::spawn(keep_alive_task(rx))))
            .await
            .unwrap();

        send_task_handle
            .send(ManagedTaskAdd::unrecoverable(tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                panic!("Task has panicked")
            })))
            .await
            .unwrap();

        handle_shutdown(main_task.await);
    }
}
