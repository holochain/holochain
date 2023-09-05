use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

pub(super) struct UiLayout {
    pub node_list: Rect,
    pub basis_table: Rect,
    pub _throughput_summary: Rect,
    pub table_extras: Rect,
    pub gauges: Vec<Rect>,
    pub bottom: Rect,
    pub time: Rect,
    pub modal: Rect,
}

pub(super) fn layout<K: Backend>(n: usize, b: usize, f: &mut Frame<K>) -> UiLayout {
    let list_len = 4;
    let table_len = b as u16 * 4 + 2;
    let stats_height = 5;
    let vsplit = Layout::default()
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
                Constraint::Length(8),
                Constraint::Length(16),
            ]
            .as_ref(),
        )
        .split(vsplit[0]);

    const MODAL_MARGIN: u16 = 3;
    let mut modal = f.size();
    modal.x += MODAL_MARGIN;
    modal.y += MODAL_MARGIN;
    modal.width -= MODAL_MARGIN;
    modal.height -= MODAL_MARGIN;

    let node_list = top_chunks[0];
    let basis_table = top_chunks[1];
    let _throughput_summary = top_chunks[2];
    let table_extras = top_chunks[3];
    let mut bottom = vsplit[1];

    bottom.y += 1;
    bottom.height -= 1;

    let w = f.size().width;
    let tw = 16;
    let time = Rect {
        x: w - tw,
        y: 0,
        width: tw,
        height: 1,
    };

    let mut gauges_rect = table_extras;
    gauges_rect.y += 1;
    gauges_rect.height -= 1;

    let gauge_heights = vec![Constraint::Length(1); n];
    let gauges = Layout::default()
        .direction(Direction::Vertical)
        .constraints(gauge_heights)
        .split(gauges_rect);

    UiLayout {
        node_list,
        basis_table,
        _throughput_summary,
        table_extras,
        gauges,
        bottom,
        time,
        modal,
    }
}
