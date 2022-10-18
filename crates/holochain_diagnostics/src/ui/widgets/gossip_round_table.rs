use std::fmt::Display;

use super::*;

pub fn gossip_round_table<Id: Display>(
    infos: &NodeInfoList<Id>,
    start_time: Instant,
    filter_zeroes: bool,
) -> Table<'static> {
    let mut currents: Vec<_> = infos
        .iter()
        .filter_map(|(n, i)| i.current_round.clone().map(|r| (n.clone(), r)))
        .collect();

    let mut metrics: Vec<_> = infos
        .iter()
        .flat_map(|(n, info)| {
            info.complete_rounds
                .clone()
                .into_iter()
                .map(move |r| (n, r))
        })
        .collect();

    currents.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    metrics.sort_unstable_by(|a, b| b.1.cmp(&a.1));

    let header = Row::new(["g", "n", "T", "dur", "#in", "#out", "in", "out", "thru"])
        .style(Style::default().add_modifier(Modifier::UNDERLINED));

    let mut rows = vec![];

    // Add current round info

    rows.extend(
        currents
            .into_iter()
            .map(|(n, metric)| render_gossip_metric_row(n, metric, start_time, true)),
    );

    // Add past round info

    rows.extend(metrics.into_iter().filter_map(|(n, info)| {
        let zero = info
            .round
            .as_ref()
            .map(|r| {
                r.throughput.op_count.incoming
                    + r.throughput.op_count.outgoing
                    + r.throughput.op_bytes.incoming
                    + r.throughput.op_bytes.outgoing
                    == 0
            })
            .unwrap_or(false);
        if filter_zeroes && zero {
            None
        } else {
            Some(render_gossip_metric_row(n.clone(), info, start_time, false))
        }
    }));

    Table::new(rows).header(header).widths(&[
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
    ])
}

fn render_gossip_metric_row<Id: Display>(
    id: Id,
    metric: RoundMetric,
    start_time: Instant,
    current: bool,
) -> Row<'static> {
    let throughput_cell = |b, d: Duration| {
        let cell = Cell::from(format!(
            "{}",
            (b as f64 * 1000. / d.as_millis() as f64).human_throughput_bytes()
        ));
        if b == 0 {
            cell.style(Style::default().fg(Color::DarkGray))
        } else {
            cell
        }
    };

    let number_cell = |v| {
        let cell = Cell::from(format!("{:>6}", v));
        if v == 0 {
            cell.style(Style::default().fg(Color::DarkGray))
        } else {
            cell
        }
    };

    let bytes_cell = |v: u32| {
        let cell = Cell::from(format!("{:>3.1}", v.human_count_bytes()));
        if v == 0 {
            cell.style(Style::default().fg(Color::DarkGray))
        } else if v >= 1_000_000 {
            cell.style(Style::default().add_modifier(Modifier::ITALIC))
        } else {
            cell
        }
    };

    let (gt, style) = match metric.gossip_type {
        GossipModuleType::ShardedRecent => (
            Cell::from("R".to_string()),
            Style::default().fg(Color::Green),
        ),
        GossipModuleType::ShardedHistorical => (
            Cell::from("H".to_string()),
            Style::default().fg(Color::Blue),
        ),
    };
    let mut cells = vec![
        gt,
        Cell::from(id.to_string()),
        Cell::from(format!(
            "{:.1?}",
            metric
                .instant
                .duration_since(TokioInstant::from(start_time))
        )),
    ];

    let dur = if current {
        metric.instant.elapsed()
    } else if let Some(round) = &metric.round {
        metric.instant.duration_since(round.start_time)
    } else {
        Duration::ZERO
    };

    cells.push({
        let style = if dur.as_millis() >= 1000 {
            Style::default().fg(Color::Red)
        } else if dur.as_millis() >= 100 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        Cell::from(format!("{:3.1?}", dur)).style(style)
    });

    if let Some(round) = metric.round {
        cells.extend([
            number_cell(round.throughput.op_count.incoming),
            number_cell(round.throughput.op_count.outgoing),
            bytes_cell(round.throughput.op_bytes.incoming),
            bytes_cell(round.throughput.op_bytes.outgoing),
            throughput_cell(
                round.throughput.op_bytes.incoming + round.throughput.op_bytes.outgoing,
                dur,
            ),
        ])
    }
    let style = if current {
        style.add_modifier(Modifier::REVERSED)
    } else {
        style
    };
    Row::new(cells).style(style)
}
