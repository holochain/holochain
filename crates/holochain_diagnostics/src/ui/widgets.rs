use super::*;

mod gossip_round_table;
pub use gossip_round_table::gossip_round_table;

pub fn ui_node_list(infos: &[NodeInfoList]) -> List<'static> {
    let nodes = infos.iter().enumerate().map(|(i, infos)| {
        let active = if infos.iter().any(|i| i.1.current_round.is_some()) {
            "*"
        } else {
            " "
        };
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

pub fn ui_basis_table<const N: usize, const B: usize>(
    underline_duration: Duration,
    state: &State<N, B>,
) -> Table<'static> {
    let header = Row::new(
        state
            .commits
            .iter()
            .enumerate()
            .map(|(i, _)| format!(" {}", i)),
    )
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
    );

    let rows = state.counts.iter().enumerate().map(|(_i, r)| {
        let cells = r.into_iter().enumerate().map(|(_, (c, t))| {
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
    });
    Table::new(rows)
        .header(header)
        .block(Block::default().borders(Borders::union(Borders::LEFT, Borders::RIGHT)))
        .widths(&[Constraint::Length(3); B])
}

pub fn ui_global_stats<const N: usize, const B: usize>(
    start_time: Instant,
    state: &State<N, B>,
) -> List<'static> {
    List::new(
        [
            format!("T:           {:<.2?}", start_time.elapsed()),
            format!("Commits:     {}", state.total_commits()),
            format!("Discrepancy: {}", state.total_discrepancy()),
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
            format!("up/down/j/k : select node"),
            format!("          0 : toggle empty gossip rounds"),
            format!("          q : Quit"),
        ]
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::TOP).title("Keymap"))
}

pub fn ui_gossip_info_table(infos: &NodeInfoList, n: usize) -> Table<'static> {
    let header = Row::new(["A", "ini", "rmt", "cmp", "err"])
        .style(Style::default().add_modifier(Modifier::UNDERLINED));

    Table::new(
        infos
            .iter()
            .map(|(i, info)| ui_gossip_info_row(info, n == *i))
            .collect::<Vec<_>>(),
    )
    .header(header)
    .widths(&[
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        // Constraint::Length(5),
        Constraint::Percentage(100),
    ])
}

fn ui_gossip_info_row(info: &NodeInfo, own: bool) -> Row<'static> {
    let active = if info.current_round.is_some() {
        "*"
    } else {
        " "
    }
    .to_string();
    let rounds = info
        .complete_rounds
        .iter()
        .map(|i| format!("{}", i.duration().as_millis()))
        .rev()
        .join(" ");
    // let latency = format!("{:3}", *info.latency_micros / 1000.0);
    if own {
        Row::new(vec![
            active,
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            // latency,
            rounds,
        ])
    } else {
        Row::new(vec![
            active,
            info.initiates.len().to_string(),
            info.remote_rounds.len().to_string(),
            info.complete_rounds.len().to_string(),
            info.errors.len().to_string(),
            // latency,
            rounds,
        ])
    }
}
