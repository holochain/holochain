use holochain::prelude::{gossip::sharded_gossip::RoundThroughput, metrics::PeerAgentHistory};

use super::*;

pub mod gossip_agent_history;
pub mod gossip_region_table;
pub mod gossip_round_table;

pub fn ui_node_list(nodes: impl Iterator<Item = (usize, bool)>) -> List<'static> {
    let nodes = nodes.map(|(i, active)| {
        let active = if active { "*" } else { " " };
        format!("{}C{}", active, i)
    });
    List::new(
        ["<G>".to_string()]
            .into_iter()
            .chain(nodes)
            .map(ListItem::new)
            .collect::<Vec<_>>(),
    )
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
}

pub fn ui_gossip_progress_gauge(throughput: RoundThroughput) -> Gauge<'static> {
    let n = throughput.op_bytes.incoming;
    let d = throughput.expected_op_bytes.incoming;

    if d > 0 {
        let r = n as f64 / d as f64;
        let mut style = Style::default().fg(Color::DarkGray).bg(Color::Cyan);
        if r > 1.0 {
            style = style
                .add_modifier(Modifier::ITALIC)
                .add_modifier(Modifier::BOLD)
        }
        let clamped = r.min(1.0).max(0.0);
        Gauge::default()
            .label(format!(
                "{} / {} ({:3.1}%)",
                n.human_count_bytes(),
                d.human_count_bytes(),
                r * 100.0,
            ))
            .ratio(clamped)
            .gauge_style(style)
    } else {
        let style = Style::default().fg(Color::Green).bg(Color::Blue);
        Gauge::default()
            .label("complete")
            .ratio(1.0)
            .gauge_style(style)
    }
}

pub fn ui_basis_table(underline_duration: Duration, counts: LinkCountsRef) -> Table<'static> {
    let mut num_bases = 0;

    let rows: Vec<_> = counts
        .iter()
        .enumerate()
        .map(|(_i, r)| {
            if num_bases > 0 {
                assert_eq!(r.len(), num_bases);
            } else {
                num_bases = r.len();
            }

            let cells = r.iter().enumerate().map(|(_, (c, t))| {
                let val = (*c).min(MAX_COUNT);
                let mut style = if val == 0 {
                    Style::default().fg(Color::Green)
                } else if val < YELLOW_THRESHOLD {
                    Style::default().fg(Color::Yellow)
                } else if val < RED_THRESHOLD {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Magenta)
                };
                if t.elapsed() < underline_duration {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
                Cell::from(format!("{:3}", val)).style(style)
            });
            Row::new(cells)
        })
        .collect();

    let header = Row::new((0..num_bases).map(|i| format!(" {}", i))).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
    );

    Table::new(rows).header(header)
}

pub fn ui_global_stats(start_time: Instant, state: &impl ClientState) -> List<'static> {
    List::new(
        [
            format!(
                "T:           {:<.2?}",
                state.time().duration_since(start_time)
            ),
            format!("Commits:     {}", state.total_commits()),
        ]
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::TOP).title("Stats"))
}

pub fn ui_keymap() -> List<'static> {
    List::new(
        [
            "↑/↓/j/k : select node".to_string(),
            "      n : add new Node".to_string(),
            "      e : commit new Entry".to_string(),
            "      g : force Gossip to reawaken".to_string(),
            "      x : eXchange peer info across all nodes".to_string(),
            "      c : Clear garbage from background buffer".to_string(),
            "      0 : toggle empty gossip rounds".to_string(),
            "      q : Quit".to_string(),
        ]
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::TOP).title("Keymap"))
}
