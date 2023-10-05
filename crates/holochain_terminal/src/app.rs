#[derive(Debug)]
pub struct App {
    /// Whether the app should keep running
    running: bool,

    /// The current tab
    tab_index: usize,

    /// The number of tabs
    tab_count: usize,
}

impl App {
    pub fn new(tab_count: usize) -> Self {
        Self {
            running: true,
            tab_index: 0,
            tab_count,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn tab_index(&self) -> usize {
        self.tab_index
    }

    pub fn incr_tab_index(&mut self) -> usize {
        self.tab_index = (self.tab_index + 1) % self.tab_count;
        self.tab_index
    }

    pub fn decr_tab_index(&mut self) -> usize {
        self.tab_index -= 1;
        if self.tab_index < 0 {
            self.tab_index += self.tab_count;
        }
        self.tab_index
    }
}
