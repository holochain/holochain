use ratatui::{prelude::*, widgets::*};

pub fn show_message<B: Backend>(message: &str, frame: &mut Frame<B>, rect: Rect) {
    let p = Paragraph::new(message).block(Block::default());
    frame.render_widget(p, rect);
}
