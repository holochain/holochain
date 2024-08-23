use crate::cli::Args;
use ratatui::layout::{Constraint, Flex, Rect};
use ratatui::prelude::{Alignment, Buffer, Direction, Layout, Style, Stylize, Widget};
use ratatui::style::Color;
use ratatui::widgets::{Block, List, ListItem};
use std::sync::Arc;

pub struct HomeWidget {
    args: Arc<Args>,
}

impl HomeWidget {
    pub fn new(args: Arc<Args>) -> Self {
        Self { args }
    }
}

impl Widget for HomeWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .direction(Direction::Vertical)
            .split(area);

        let header = Block::default()
            .title("Welcome to the Holochain Terminal")
            .style(Style::default().bold())
            .title_alignment(Alignment::Center);

        header.render(layout[0], buf);

        let help = Block::default()
            .title("Use the TAB key to navigate between screens and press ESC to exit the terminal")
            .title_alignment(Alignment::Center);

        help.render(layout[1], buf);

        let [args_layout] = Layout::horizontal([Constraint::Percentage(40)])
            .flex(Flex::Center)
            .areas(area);

        let [args_layout] = Layout::vertical([Constraint::Length(5)])
            .flex(Flex::Center)
            .areas(args_layout);

        let args = List::new(vec![
            ListItem::new(format!(
                "- Admin URL    : {}",
                match self.args.admin_url {
                    Some(ref admin_url) => admin_url.to_string(),
                    None => "not configured".to_string(),
                }
            ))
            .style(Style::default().italic().fg(Color::Gray)),
            ListItem::new(format!(
                "- Boostrap URL : {}",
                match self.args.bootstrap_url {
                    Some(ref bootstrap_url) => bootstrap_url.to_string(),
                    None => "not configured".to_string(),
                }
            ))
            .style(Style::default().italic().fg(Color::Gray)),
            ListItem::new(format!(
                "- DNA hash     : {}",
                match self.args.dna_hash {
                    Some(ref app_id) => app_id.to_string(),
                    None => "not configured".to_string(),
                }
            ))
            .style(Style::default().italic().fg(Color::Gray)),
            ListItem::new(format!(
                "- App ID       : {}",
                match self.args.app_id {
                    Some(ref app_id) => app_id.to_string(),
                    None => "not configured".to_string(),
                }
            ))
            .style(Style::default().italic().fg(Color::Gray)),
        ])
        .block(Block::default().title("Started with args:").bold().white())
        .style(Style::default().bg(Color::Black));
        args.render(args_layout, buf);
    }
}
