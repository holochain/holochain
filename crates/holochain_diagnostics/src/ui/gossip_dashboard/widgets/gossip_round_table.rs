use std::fmt::Display;

use holochain::prelude::gossip::sharded_gossip::RoundThroughput;

use super::*;

pub struct GossipRoundTableState<'a, Id: Display> {
    pub infos: &'a NodeHistories<'a, Id>,
    pub start_time: Instant,
    pub current_time: Instant,
    pub filter_zeroes: bool,
}

pub fn gossip_round_table<Id: Display>(state: &GossipRoundTableState<Id>) -> Table<'static> {
    let mut currents: Vec<_> = state
        .infos
        .iter()
        .filter_map(|(n, i)| i.current_round.clone().map(|r| (n, r)))
        .collect();

    let mut metrics: Vec<_> = state
        .infos
        .iter()
        .flat_map(|(n, info)| {
            info.completed_rounds
                .clone()
                .into_iter()
                .map(move |r| (n, r))
        })
        .collect();

    currents.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    metrics.sort_unstable_by(|a, b| b.1.cmp(&a.1));

    let header = Row::new([
        "g", "e", "n", "T", "dur", "#in", "#out", "in", "out", "thru",
    ])
    .style(Style::default().add_modifier(Modifier::UNDERLINED));

    let mut rows = vec![];

    // Add current round info

    rows.extend(currents.into_iter().map(|(n, round)| {
        render_gossip_metric_row(
            n.clone(),
            round.gossip_type,
            Instant::from(round.start_time).duration_since(state.start_time),
            state.current_time.duration_since(round.start_time.into()),
            Some(round.throughput),
            true,
            false,
        )
    }));

    // Add past round info

    rows.extend(metrics.into_iter().filter_map(|(n, round)| {
        let zero = round.throughput.op_count.incoming
            + round.throughput.op_count.outgoing
            + round.throughput.op_bytes.incoming
            + round.throughput.op_bytes.outgoing
            == 0;
        if state.filter_zeroes && zero {
            None
        } else {
            Some(render_gossip_metric_row(
                n.clone(),
                round.gossip_type,
                Instant::from(round.start_time).duration_since(state.start_time),
                round.duration(),
                Some(round.throughput),
                false,
                round.error,
            ))
        }
    }));

    Table::new(rows).header(header).widths(&[
        Constraint::Length(1),
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
    id: &Id,
    gossip_type: GossipModuleType,
    time_since_start: Duration,
    duration: Duration,
    throughput: Option<RoundThroughput>,
    is_current: bool,
    error: bool,
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
            if is_current {
                cell.style(Style::default().bg(Color::Gray))
            } else {
                cell.style(Style::default().fg(Color::Gray))
            }
        } else {
            cell
        }
    };

    let bytes_cell = |v: u32, expected: u32| {
        let cell = if expected == 0 {
            Cell::from(format!("{:>5.1}", v.human_count_bytes()))
        } else {
            Cell::from(format!(
                "{:>5.1}/{:>5.1}",
                v.human_count_bytes(),
                expected.human_count_bytes()
            ))
        };
        if v == 0 {
            cell.style(Style::default().fg(Color::Gray))
        } else if v >= 1_000_000 {
            cell.style(Style::default().add_modifier(Modifier::ITALIC))
        } else {
            cell
        }
    };

    let (gt, style) = match gossip_type {
        GossipModuleType::ShardedRecent => (
            Cell::from("R".to_string()),
            Style::default().fg(Color::Green),
        ),
        GossipModuleType::ShardedHistorical => (
            Cell::from("H".to_string()),
            Style::default().fg(Color::Blue),
        ),
    };
    let err = Cell::from(if error { "E" } else { " " });
    let mut cells = vec![
        gt,
        err,
        Cell::from(id.to_string()),
        Cell::from(format!("{:.1?}", time_since_start)),
    ];

    cells.push({
        let style = if duration.as_millis() >= 1000 {
            Style::default().fg(Color::Red)
        } else if duration.as_millis() >= 100 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        Cell::from(format!("{:3.1?}", duration)).style(style)
    });

    if let Some(tp) = throughput {
        cells.extend([
            number_cell(tp.op_count.incoming),
            number_cell(tp.op_count.outgoing),
            bytes_cell(tp.op_bytes.incoming, tp.total_region_size.incoming),
            bytes_cell(tp.op_bytes.outgoing, tp.total_region_size.outgoing),
            throughput_cell(tp.op_bytes.incoming + tp.op_bytes.outgoing, duration),
        ])
    }
    let style = if is_current {
        style.add_modifier(Modifier::REVERSED)
    } else {
        style
    };
    Row::new(cells).style(style)
}
