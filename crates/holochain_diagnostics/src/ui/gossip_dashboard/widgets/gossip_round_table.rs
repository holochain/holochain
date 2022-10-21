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

    let header = Row::new(["g", "n", "T", "dur", "#in", "#out", "in", "out", "thru"])
        .style(Style::default().add_modifier(Modifier::UNDERLINED));

    let mut rows = vec![];

    // Add current round info

    rows.extend(currents.into_iter().map(|(n, round)| {
        render_gossip_metric_row(
            state,
            n.clone(),
            round.gossip_type,
            round.start_time.into(),
            state.current_time.duration_since(round.start_time.into()),
            Some(round.current_throughput),
            true,
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
                state,
                n.clone(),
                round.gossip_type,
                round.start_time.into(),
                round.duration(),
                Some(round.throughput),
                false,
            ))
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
    state: &GossipRoundTableState<Id>,
    id: &Id,
    gossip_type: GossipModuleType,
    start_time: Instant,
    duration: Duration,
    throughput: Option<RoundThroughput>,
    is_current: bool,
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
    let mut cells = vec![
        gt,
        Cell::from(id.to_string()),
        Cell::from(format!(
            "{:.1?}",
            start_time // metric
                       //     .instant
                       //     .duration_since(TokioInstant::from(state.start_time))
        )),
    ];

    // let dur = if is_current {
    //     state.current_time.duration_since(metric.instant.into())
    // } else if let Some(round) = &metric.round {
    //     metric.instant.duration_since(round.start_time)
    // } else {
    //     Duration::ZERO
    // };

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
            bytes_cell(tp.op_bytes.incoming),
            bytes_cell(tp.op_bytes.outgoing),
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
