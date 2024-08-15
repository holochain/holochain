use ratatui::{prelude::*, widgets::*};

pub fn show_message(message: &str, area: Rect, buf: &mut Buffer) {
    let p = Paragraph::new(message).block(Block::default());
    p.render(area, buf);
}
