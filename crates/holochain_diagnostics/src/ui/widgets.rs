use super::*;

mod gossip_round_table;
pub use gossip_round_table::gossip_round_table;

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
            format!("T:           {:<.2?}", start_time.elapsed()),
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
            format!("↑/↓/j/k : select node"),
            format!("      n : add new Node"),
            format!("      x : eXchange peer info across all nodes"),
            format!("      c : Clear garbage from background buffer"),
            format!("      0 : toggle empty gossip rounds"),
            format!("      q : Quit"),
        ]
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::TOP).title("Keymap"))
}

pub fn ui_gossip_info_table(infos: &NodeInfoList<usize>, n: usize) -> Table<'static> {
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
            // "✓".to_string(),
            "·".to_string(),
            // active,
            "·".to_string(),
            "·".to_string(),
            "·".to_string(),
            "·".to_string(),
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
