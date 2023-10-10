use crate::cli::Args;
use crate::event::ScreenEvent;

#[derive(Debug)]
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
    args: Args,
}

impl App {
    pub fn new(args: Args, tab_count: usize) -> Self {
        Self {
            running: true,
            tab_index: 0,
            tab_count,
            pending_events: vec![ScreenEvent::Refresh],
            args,
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

    pub fn args(&self) -> &Args {
        &self.args
    }
}
