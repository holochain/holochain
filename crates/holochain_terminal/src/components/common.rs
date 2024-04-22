use ratatui::{prelude::*, widgets::*};

pub fn show_message(message: &str, frame: &mut Frame, rect: Rect) {
    let p = Paragraph::new(message).block(Block::default());
    frame.render_widget(p, rect);
}
