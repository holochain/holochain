pub use inner::*;

#[cfg(any(test, feature = "test_utils"))]
mod inner {
    /// Force all DhtOps without enough validation receipts to be republished.
    #[derive(Clone)]
    pub struct ForcePublishSender(tokio::sync::broadcast::Sender<()>);

    /// Force publish handler for the publish dht ops workflow.
    pub struct ForcePublishHandler(tokio::sync::broadcast::Receiver<()>);

    impl ForcePublishSender {
        /// Create a new force publish sender.
        pub fn new() -> Self {
            let (tx, _) = tokio::sync::broadcast::channel(1);
            Self(tx)
        }

        /// Create a new handler for this sender.
        pub fn handler(&self) -> ForcePublishHandler {
            ForcePublishHandler(self.0.subscribe())
        }

        /// Force the publish workflow to publish all DhtOps that have
        /// not yet received enough validation receipts.
        pub fn force(&self) {
            if self.0.send(()).is_err() {
                tracing::warn!(
                    "Tried to force publish when the publish workflow is already closed"
                );
            }
        }
    }

    impl ForcePublishHandler {
        /// Should we force publish all DhtOps that have
        /// not yet received enough validation receipts?
        pub(in super::super) fn forced(&mut self) -> bool {
            !matches!(
                self.0.try_recv(),
                Err(tokio::sync::broadcast::error::TryRecvError::Empty)
                    | Err(tokio::sync::broadcast::error::TryRecvError::Closed)
            )
        }
    }

    impl Default for ForcePublishSender {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Default noop for tests that don't need this.
    impl Default for ForcePublishHandler {
        fn default() -> Self {
            Self(tokio::sync::broadcast::channel(1).1)
        }
    }
}

#[cfg(not(any(test, feature = "test_utils")))]
mod inner {
    /// Noop, use feature test_utils.
    #[derive(Clone)]
    pub struct ForcePublishSender;

    /// Noop, use feature test_utils.
    pub struct ForcePublishHandler;

    impl ForcePublishSender {
        /// Noop, use feature test_utils.
        pub fn new() -> Self {
            Self
        }

        /// Noop, use feature test_utils.
        pub fn handler(&self) -> ForcePublishHandler {
            ForcePublishHandler
        }

        /// Noop, use feature test_utils.
        pub fn force(&self) {}
    }

    impl ForcePublishHandler {
        /// Noop, use feature test_utils.
        pub(in super::super) fn forced(&mut self) -> bool {
            false
        }
    }

    impl Default for ForcePublishSender {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Default for ForcePublishHandler {
        fn default() -> Self {
            Self
        }
    }
}
