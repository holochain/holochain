use holochain_types::app::InstalledAppId;
use holochain_types::prelude::Signal;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::broadcast;

// Number of signals in buffer before we start dropping them.
// 64 gives us a good burst buffer incase multiple threads are
// sending signals at the same time and we need to catch up,
// but not so many that we have to be overly concerned about
// the memory usage implications.
const SIGNAL_BUFFER_SIZE: usize = 64;

#[derive(Debug, Clone)]
pub struct AppBroadcast {
    channels: Arc<parking_lot::Mutex<HashMap<InstalledAppId, broadcast::Sender<Signal>>>>,
}

impl AppBroadcast {
    pub(crate) fn new() -> Self {
        Self {
            channels: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }

    /// Create a signal sender for a specific installed app.
    ///
    /// The app does not actually need to be installed to call this and it does not need to be
    /// called before subscribing to signals.
    pub(crate) fn create_send_handle(
        &self,
        installed_app_id: InstalledAppId,
    ) -> broadcast::Sender<Signal> {
        match self.channels.lock().entry(installed_app_id) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => e.insert(broadcast::channel(SIGNAL_BUFFER_SIZE).0).clone(),
        }
    }

    /// Subscribe to signals for a specific installed app.
    ///
    /// The app does not actually need to be installed to call this and the sdner does not need to
    /// be created before subscribing.
    pub(crate) fn subscribe(
        &self,
        installed_app_id: InstalledAppId,
    ) -> broadcast::Receiver<Signal> {
        match self.channels.lock().entry(installed_app_id) {
            Entry::Occupied(e) => e.get().subscribe(),
            Entry::Vacant(e) => {
                let (tx, rx) = broadcast::channel(SIGNAL_BUFFER_SIZE);
                e.insert(tx);

                rx
            }
        }
    }

    /// Given a list of currently installed apps, retain only the channels for those apps.
    /// This is useful for cleaning up channels for apps that have been uninstalled.
    pub(crate) fn retain(&self, installed_apps: HashSet<InstalledAppId>) {
        self.channels
            .lock()
            .retain(|k, _| installed_apps.contains(k));
    }

    #[cfg(test)]
    fn keys(&self) -> Vec<InstalledAppId> {
        self.channels.lock().keys().cloned().collect()
    }
}

impl Default for AppBroadcast {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ExternIO;
    use fixt::prelude::*;
    use hdk::prelude::CellIdFixturator;
    use hdk::prelude::ZomeNameFixturator;
    use holochain_zome_types::signal::AppSignal;

    #[tokio::test]
    async fn create_send_handle_and_broadcast() {
        let app_broadcast = AppBroadcast::new();
        let installed_app_id: InstalledAppId = "test".into();

        // Create sender first
        let tx = app_broadcast.create_send_handle(installed_app_id.clone());

        // Now subscribe
        let mut rx = app_broadcast.subscribe(installed_app_id.clone());

        // Send and receive the signal
        let signal = Signal::App {
            cell_id: fixt!(CellId),
            zome_name: fixt!(ZomeName),
            signal: AppSignal::new(ExternIO::from(vec![])),
        };
        tx.send(signal.clone()).unwrap();
        let received_signal = rx.recv().await.unwrap();

        assert_eq!(signal, received_signal);
    }

    #[tokio::test]
    async fn subscribe_and_broadcast() {
        let app_broadcast = AppBroadcast::new();
        let installed_app_id: InstalledAppId = "test".into();

        // Subscribe first
        let mut rx = app_broadcast.subscribe(installed_app_id.clone());

        // Now create sender
        let tx = app_broadcast.create_send_handle(installed_app_id.clone());

        // Send and receive the signal
        let signal = Signal::App {
            cell_id: fixt!(CellId),
            zome_name: fixt!(ZomeName),
            signal: AppSignal::new(ExternIO::from(vec![])),
        };
        tx.send(signal.clone()).unwrap();
        let received_signal = rx.recv().await.unwrap();

        assert_eq!(signal, received_signal);
    }

    #[tokio::test]
    async fn multiple_senders_and_subscribers() {
        let app_broadcast = AppBroadcast::new();
        let installed_app_id: InstalledAppId = "test".into();

        // Create sender 1 and subscriber 1
        let tx_1 = app_broadcast.create_send_handle(installed_app_id.clone());
        let mut rx_1 = app_broadcast.subscribe(installed_app_id.clone());

        // Create sender 2 and subscriber 2
        let tx_2 = app_broadcast.create_send_handle(installed_app_id.clone());
        let mut rx_2 = app_broadcast.subscribe(installed_app_id.clone());

        let signal_1 = Signal::App {
            cell_id: fixt!(CellId),
            zome_name: fixt!(ZomeName),
            signal: AppSignal::new(ExternIO::from(vec![])),
        };
        tx_1.send(signal_1.clone()).unwrap();

        let signal_1_rcv_1 = rx_1.recv().await.unwrap();
        let signal_1_rcv_2 = rx_2.recv().await.unwrap();
        assert_eq!(signal_1, signal_1_rcv_1);
        assert_eq!(signal_1, signal_1_rcv_2);

        let signal_2 = Signal::App {
            cell_id: fixt!(CellId),
            zome_name: fixt!(ZomeName),
            signal: AppSignal::new(ExternIO::from(vec![])),
        };
        tx_2.send(signal_2.clone()).unwrap();

        let signal_2_rcv_1 = rx_1.recv().await.unwrap();
        let signal_2_rcv_2 = rx_2.recv().await.unwrap();
        assert_eq!(signal_2, signal_2_rcv_1);
        assert_eq!(signal_2, signal_2_rcv_2);
    }

    #[tokio::test]
    async fn clean_up_unused_senders() {
        let app_broadcast = AppBroadcast::new();

        let installed_app_id_1: InstalledAppId = "test 1".into();
        let _tx_1 = app_broadcast.create_send_handle(installed_app_id_1.clone());

        let installed_app_id_2: InstalledAppId = "test 2".into();
        let _tx_2 = app_broadcast.create_send_handle(installed_app_id_2.clone());

        assert_eq!(2, app_broadcast.keys().len());

        let mut still_installed = HashSet::new();
        still_installed.insert(installed_app_id_1.clone());
        app_broadcast.retain(still_installed);

        assert_eq!(1, app_broadcast.keys().len());
        assert_eq!(vec![installed_app_id_1], app_broadcast.keys());
    }
}
