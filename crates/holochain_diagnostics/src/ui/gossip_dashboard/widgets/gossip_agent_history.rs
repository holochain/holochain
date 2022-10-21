use super::*;

pub fn ui_gossip_agent_history_table(
    infos: Vec<(usize, &PeerAgentHistory)>,
    n: usize,
) -> Table<'static> {
    let header = Row::new(["A", "ini", "acc", "✅", "❌"])
        .style(Style::default().add_modifier(Modifier::UNDERLINED));

    Table::new(
        infos
            .iter()
            .map(|(i, info)| row(info, n == *i))
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

fn row(info: &PeerAgentHistory, own: bool) -> Row<'static> {
    let active = if info.current_round { "*" } else { " " }.to_string();

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
        ])
    } else {
        Row::new(vec![
            active,
            info.initiates.len().to_string(),
            info.accepts.len().to_string(),
            info.successes.len().to_string(),
            info.errors.len().to_string(),
            // latency,
        ])
    }
}
