use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

pub(super) struct UiLayout {
    pub node_list: Rect,
    pub get_table: Rect,
    pub gossip_table: Rect,
    pub stats: Rect,
    pub time: Rect,
}

pub(super) fn layout<K: Backend>(n: usize, b: usize, f: &mut Frame<K>) -> UiLayout {
    let list_len = 4;
    let table_len = b as u16 * 4 + 2;
    let stats_height = 5;
    let mut vsplit = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((n + 1) as u16),
            Constraint::Length(stats_height),
        ])
        .vertical_margin(1)
        .split(f.size());

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(list_len),
                Constraint::Length(table_len),
                Constraint::Length(16),
            ]
            .as_ref(),
        )
        .split(vsplit[0]);

    vsplit[1].y += 1;
    vsplit[1].height -= 1;

    let w = f.size().width;
    let tw = 16;
    let time = Rect {
        x: w - tw,
        y: 0,
        width: tw,
        height: 1,
    };

    UiLayout {
        node_list: top_chunks[0],
        get_table: top_chunks[1],
        gossip_table: top_chunks[2],
        stats: vsplit[1],
        time,
    }
}
