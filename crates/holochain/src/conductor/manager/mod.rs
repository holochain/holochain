mod error;
pub use error::*;

use futures::stream::FuturesUnordered;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::stream::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::*;

const CHANNEL_SIZE: usize = 1000;

pub(crate) type ManagedTaskHandle = JoinHandle<ManagedTaskResult>;
pub(crate) type TaskManagerRunHandle = JoinHandle<()>;

pub(crate) type OnDeath =
    Box<dyn FnOnce(ManagedTaskResult) -> Option<ManagedTaskAdd> + Send + Sync>;

/// A message sent to the TaskManager, registering a closure to run upon
/// completion of a task
pub(crate) struct ManagedTaskAdd {
    handle: ManagedTaskHandle,
    // TODO: reevaluate wether this should be a callback
    // This is probably not a great way to do this.
    // The task needs to check the error but then it probably needs to be able to restart itself.
    // If we use a callback then we need to be able to restart the task without duplicating all the start code.
    // We also might need some state because say a task dies 5 times, maybe you restart it 4 times but 5 you hard error or something.
    // The TaskManager could store some context like number of times killed but then we need to have unique managed tasks.
    on_death: Option<OnDeath>,
}

impl ManagedTaskAdd {
    pub(crate) fn new(handle: ManagedTaskHandle, on_death: OnDeath) -> Self {
        let on_death = Some(on_death);
        ManagedTaskAdd { handle, on_death }
    }
}

// TODO: implement, move into task that loops and select!s
struct TaskManager {
    stream: FuturesUnordered<ManagedTaskAdd>,
}

impl TaskManager {
    fn new() -> Self {
        let stream = FuturesUnordered::new();
        TaskManager { stream }
    }
}

// FIXME I'm not sure if this is correct please review
impl Future for ManagedTaskAdd {
    type Output = (Option<OnDeath>, ManagedTaskResult);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let p = std::pin::Pin::new(&mut self.handle);
        match JoinHandle::poll(p, cx) {
            Poll::Ready(r) => {
                Poll::Ready((self.on_death.take(), r.unwrap_or_else(|e| Err(e.into()))))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

pub(crate) fn spawn_task_manager() -> (mpsc::Sender<ManagedTaskAdd>, TaskManagerRunHandle) {
    let (send, recv) = mpsc::channel(CHANNEL_SIZE);
    (send, tokio::spawn(run(recv)))
}

async fn run(mut new_task_channel: mpsc::Receiver<ManagedTaskAdd>) {
    let mut task_manager = TaskManager::new();
    // Need to have atleast on item in the stream or it will exit early
    if let Some(new_task) = new_task_channel.recv().await {
        task_manager.stream.push(new_task);
    } else {
        error!("All senders to task manager were dropped before starting");
        return;
    }
    loop {
        let new_task_to_spawn = tokio::select! {
            Some(new_task) = new_task_channel.recv() => {
                task_manager.stream.push(new_task);
                None
            }
            result = task_manager.stream.next() => match result {
                Some((Some(on_death), result)) => handle_complete_task(on_death, result),
                None => break,
                _ => None,
            }
        };
        if let Some(new_task) = new_task_to_spawn {
            task_manager.stream.push(new_task)
        }
    }
}

fn handle_complete_task(
    on_death: OnDeath,
    complete_task: ManagedTaskResult,
) -> Option<ManagedTaskAdd> {
    on_death(complete_task)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::error::ConductorError;
    use anyhow::Result;

    #[tokio::test]
    async fn spawn_and_handle_dying_task() -> Result<()> {
        let (mut send_task_handle, main_task) = spawn_task_manager();
        let handle = tokio::spawn(async {
            Err(ConductorError::Todo("This task gotta die".to_string()).into())
        });
        let handle = ManagedTaskAdd::new(
            handle,
            Box::new(|result| match result {
                Ok(_) => panic!("Task should have died"),
                Err(ManagedTaskError::Conductor(ConductorError::Todo(_))) => {
                    let handle = tokio::spawn(async { Ok(()) });
                    let handle = ManagedTaskAdd::new(handle, Box::new(|_| None));
                    Some(handle)
                }
                _ => None,
            }),
        );
        // Check that the main task doesn't close staright away
        let main_handle = tokio::spawn(main_task);
        tokio::time::delay_for(std::time::Duration::from_secs(2)).await;

        // Now send the handle
        if let Err(_) = send_task_handle.send(handle).await {
            panic!("Failed to send the handle");
        }
        main_handle.await??;
        Ok(())
    }
}
