//! Manages the spawning of tasks which process the various work queues in
//! the system, as well as notifying subsequent queue processors to pick up the
//! work that was left off.
//!
//! The following table lays out the queues and the workflows that consume them,
//! as well as the follow-up workflows. A "source" queue is a database which
//! feeds data to the workflow, and a "destination" queue is a database which
//! said workflow writes to as part of its processing of its source queue.
//!
//! When a consumer has exhausted its queue, it may notify another consumer
//! that it now has work to do, because this consumer has placed work on
//! another's queue.
//!
//! | workflow        | source queue     | dest. queue      | notifies        |
//! |-----------------|------------------|------------------|-----------------|
//! | ProduceDhtOps   | ChainSequence    | IntegrationQueue | IntegrateDhtOps |
//! | SysValidation   | ValidationQueue  | ValidationQueue  | AppValidation   |
//! | AppValidation   | ValidationQueue  | ValidationQueue  | IntegrateDhtOps |
//! | IntegrateDhtOps | IntegrationQueue | IntegratedDhtOps | Publish         |
//!
//! Implicitly, every workflow also writes to its own source queue, i.e. to
//! remove the item it has just processed.

use tokio::sync::mpsc;

/// The means of nudging a queue consumer to tell it to look for more work
#[derive(Clone)]
struct Waker(mpsc::Sender<()>);

/// The receiving side of a Waker channel
type Listener = mpsc::Receiver<()>;

impl Waker {
    /// Create a new channel for waking a consumer
    ///
    /// The channel buffer is set to 1 to ensure that the consumer does not
    /// have to be concerned with draining the channel in case it has received
    /// multiple wake signals.
    pub fn new() -> (Self, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel(1);
        (Self(tx), rx)
    }

    /// Lazily nudge the consumer task, ignoring the case where the consumer
    /// already has a pending wakeup signal
    pub fn wake(&mut self) -> Result<(), QueueWakerClosedError> {
        match self.0.try_send(()) {
            Err(mpsc::error::TrySendError::Closed(_)) => Err(QueueWakerClosedError),
            Err(mpsc::error::TrySendError::Full(_)) => Ok(()),
            Ok(()) => Ok(()),
        }
    }
}

pub struct QueueWakerClosedError;

/// Spawns several long-running tasks which are responsible for processing work
/// which shows up on various databases.
pub async fn spawn_queue_consumer_tasks() {
    let (tx_integration, rx_integration) = Waker::new();

}
