use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use holochain_types::app::InstalledAppId;
use holochain_types::prelude::Signal;

#[derive(Debug, Clone)]
pub struct AppBroadcast {
    channels: Arc<parking_lot::Mutex<HashMap<InstalledAppId, broadcast::Sender<Signal>>>>
}
