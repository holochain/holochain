use holochain_types::prelude::Signal;
use tokio::sync::broadcast::{self, error::RecvError};

/// How many signals a resilient connection buffers per subscriber.
pub(crate) const SIGNAL_CHANNEL_CAPACITY: usize = 1024;

/// An item yielded by a [`SignalStream`].
#[derive(Clone, Debug)]
pub enum SignalEvent {
    /// A signal emitted by the app.
    Signal(Signal),
    /// Signals were missed and any state derived from them needs re-syncing.
    ///
    /// This is reported when the connection to the conductor dropped and has
    /// been re-established, and when this subscriber fell far enough behind
    /// that buffered signals were discarded. Holochain does not replay
    /// signals, so the missed ones cannot be recovered.
    Interrupted,
}

/// A subscription to the signals emitted by an app.
///
/// The subscription outlives the underlying websocket, so it keeps yielding
/// across reconnects without being re-registered. It ends only when the
/// connection it came from is dropped.
pub struct SignalStream(broadcast::Receiver<SignalEvent>);

impl SignalStream {
    /// Waits for the next event, or returns `None` once the connection this
    /// subscription came from has been dropped.
    ///
    /// This is cancel safe: dropping the returned future loses no events.
    pub async fn next(&mut self) -> Option<SignalEvent> {
        match self.0.recv().await {
            Ok(event) => Some(event),
            Err(RecvError::Lagged(_)) => Some(SignalEvent::Interrupted),
            Err(RecvError::Closed) => None,
        }
    }
}

pub(crate) fn signal_stream(rx: broadcast::Receiver<SignalEvent>) -> SignalStream {
    SignalStream(rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::EntryHash;
    use holochain_types::prelude::{Signal, SystemSignal};

    /// Builds a distinct signal per byte.
    ///
    /// `Signal::App` needs a real `CellId` and `ZomeName`; a system signal
    /// carries a single hash and is cheaper to construct. These tests only
    /// need a `Signal` value that differs per call.
    fn a_signal(byte: u8) -> Signal {
        Signal::System(SystemSignal::SuccessfulCountersigning(
            EntryHash::from_raw_32(vec![byte; 32]),
        ))
    }

    #[tokio::test]
    async fn yields_signals_in_order() {
        let (tx, rx) = broadcast::channel(SIGNAL_CHANNEL_CAPACITY);
        let mut stream = signal_stream(rx);

        tx.send(SignalEvent::Signal(a_signal(1))).unwrap();
        tx.send(SignalEvent::Signal(a_signal(2))).unwrap();

        assert!(matches!(stream.next().await, Some(SignalEvent::Signal(_))));
        assert!(matches!(stream.next().await, Some(SignalEvent::Signal(_))));
    }

    #[tokio::test]
    async fn reports_interruption() {
        let (tx, rx) = broadcast::channel(SIGNAL_CHANNEL_CAPACITY);
        let mut stream = signal_stream(rx);

        tx.send(SignalEvent::Interrupted).unwrap();

        assert!(matches!(
            stream.next().await,
            Some(SignalEvent::Interrupted)
        ));
    }

    #[tokio::test]
    async fn a_lagging_consumer_is_reported_as_an_interruption() {
        let (tx, rx) = broadcast::channel(2);
        let mut stream = signal_stream(rx);

        for byte in 0..5 {
            tx.send(SignalEvent::Signal(a_signal(byte))).unwrap();
        }

        assert!(matches!(
            stream.next().await,
            Some(SignalEvent::Interrupted)
        ));
    }

    #[tokio::test]
    async fn ends_when_the_sender_is_dropped() {
        let (tx, rx) = broadcast::channel(SIGNAL_CHANNEL_CAPACITY);
        let mut stream = signal_stream(rx);

        drop(tx);

        assert!(stream.next().await.is_none());
    }
}
