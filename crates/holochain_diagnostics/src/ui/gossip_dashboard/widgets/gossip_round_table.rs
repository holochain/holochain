//! Displays a table of info about each gossip round a node has participated in.

use crate::dht::prelude::RegionData;
use std::fmt::Display;

use super::*;

pub struct GossipRoundTableState<'a, Id: Display + Clone> {
    pub rounds: &'a NodeRounds<'a, Id>,
    pub start_time: Instant,
    pub current_time: Instant,
    pub filter_zeroes: bool,
    pub table_state: &'a TableState,
}

pub fn gossip_round_table<Id: Display + Clone>(
    state: &GossipRoundTableState<Id>,
) -> Table<'static> {
    let header = Row::new(["g", "e", "n", "id", "T", "dur", "#in", "#out", "in", "out"])
        .style(Style::default().add_modifier(Modifier::UNDERLINED));

    let mut rows = vec![];

    // Add current round info

    rows.extend(
        state
            .rounds
            .currents
            .iter()
            .enumerate()
            .map(|(i, (n, round))| {
                render_gossip_metric_row(
                    &n,
                    &round.id,
                    round.gossip_type,
                    Instant::from(round.start_time).duration_since(state.start_time),
                    state.current_time.duration_since(round.start_time.into()),
                    &round.region_diffs,
                    true,
                    Some(i) == state.table_state.selected(),
                    false,
                )
            }),
    );

    // Add past round info

    rows.extend(
        state
            .rounds
            .completed
            .iter()
            .enumerate()
            .filter_map(|(i, (n, round))| {
                let zero = round
                    .region_diffs
                    .as_ref()
                    .map(|(ours, theirs)| ours.is_empty() && theirs.is_empty())
                    .unwrap_or(false);
                if state.filter_zeroes && zero {
                    None
                } else {
                    Some(render_gossip_metric_row(
                        &n,
                        &round.id,
                        round.gossip_type,
                        Instant::from(round.start_time).duration_since(state.start_time),
                        round.duration(),
                        &round.region_diffs,
                        false,
                        Some(i) == state.table_state.selected(),
                        round.error,
                    ))
                }
            }),
    );

    Table::new(rows).header(header).widths(&[
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Min(3),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
        Constraint::Percentage(100 / 8),
    ])
}

#[allow(clippy::too_many_arguments)]
fn render_gossip_metric_row<Id: Display>(
    node_id: &Id,
    round_id: &String,
    gossip_type: GossipModuleType,
    time_since_start: Duration,
    duration: Duration,
    region_diffs: &RegionDiffs,
    is_current: bool,
    is_selected: bool,
    error: bool,
) -> Row<'static> {
    let number_cell = |v: u32| {
        let cell = Cell::from(format!("{:>6}", v));
        if v == 0 {
            // if is_current {
            //     cell.style(Style::default().bg(Color::Gray))
            // } else {
            cell.style(Style::default().fg(Color::DarkGray))
            // }
        } else {
            cell
        }
    };

    let bytes_cell = |v: u32| {
        let cell = Cell::from(format!("{:>5.1}", v.human_count_bytes()));
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
        Cell::from(node_id.to_string()),
        Cell::from(round_id.to_string()),
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

    if let Some((ours, theirs)) = region_diffs {
        let outgoing: RegionData = ours.iter().map(|r| r.data.clone()).sum();
        let incoming: RegionData = theirs.iter().map(|r| r.data.clone()).sum();
        cells.extend([
            number_cell(incoming.count),
            number_cell(outgoing.count),
            bytes_cell(incoming.size),
            bytes_cell(outgoing.size),
        ])
    }
    let style = if is_current {
        style
            .add_modifier(Modifier::UNDERLINED)
            .add_modifier(Modifier::ITALIC)
    } else {
        style
    };
    let style = if is_selected {
        style.add_modifier(Modifier::REVERSED)
    } else {
        style
    };
    Row::new(cells).style(style)
}
