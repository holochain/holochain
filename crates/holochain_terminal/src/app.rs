use crate::cli::Args;
use crate::client::{AdminClient, AppClient};
use crate::event::ScreenEvent;
use kitsune_p2p_types::dependencies::tokio;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct App {
    /// Whether the app should keep running
    running: bool,

    /// The current tab
    tab_index: usize,

    /// The number of tabs
    tab_count: usize,

    /// Events to be processed by the current screen
    pending_events: Vec<ScreenEvent>,

    /// The command line args provided to the terminal on launch
    args: Arc<Args>,

    /// An admin client if the `admin_url` flag was provided
    #[allow(dead_code)]
    admin_client: Option<Arc<Mutex<AdminClient>>>,

    /// An app client if the `admin_url` flag was provided
    app_client: Option<Arc<Mutex<AppClient>>>,
}

impl App {
    pub fn new(
        args: Args,
        admin_client: Option<AdminClient>,
        app_client: Option<AppClient>,
        tab_count: usize,
    ) -> Self {
        Self {
            running: true,
            tab_index: 0,
            tab_count,
            pending_events: vec![ScreenEvent::Refresh],
            args: Arc::new(args),
            admin_client: admin_client.map(|c| Arc::new(Mutex::new(c))),
            app_client: app_client.map(|c| Arc::new(Mutex::new(c))),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn push_event(&mut self, event: ScreenEvent) {
        self.pending_events.push(event);
    }

    pub fn drain_events(&mut self) -> Vec<ScreenEvent> {
        self.pending_events.drain(0..).collect()
    }

    pub fn tab_index(&self) -> usize {
        self.tab_index
    }

    pub fn incr_tab_index(&mut self) -> usize {
        self.tab_index = (self.tab_index + 1) % self.tab_count;
        self.tab_index
    }

    pub fn decr_tab_index(&mut self) -> usize {
        self.tab_index = (self.tab_index + self.tab_count - 1) % self.tab_count;
        self.tab_index
    }

    pub fn args(&self) -> Arc<Args> {
        self.args.clone()
    }

    #[allow(dead_code)]
    pub fn admin_client(&mut self) -> Option<Arc<Mutex<AdminClient>>> {
        self.admin_client.clone()
    }

    pub fn app_client(&mut self) -> Option<Arc<Mutex<AppClient>>> {
        self.app_client.clone()
    }
}
