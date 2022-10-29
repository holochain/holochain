use std::fmt::Display;

use holochain::prelude::{dht::region::Region, gossip::sharded_gossip::RoundThroughput};

use super::*;

pub struct GossipRegionTableState<'a> {
    pub regions: &'a Vec<Region>,
}

pub fn gossip_region_table(state: &GossipRegionTableState) -> Table<'static> {
    let header = Row::new(["coords", "#", "size" /* , "hash"*/])
        .style(Style::default().add_modifier(Modifier::UNDERLINED));

    let rows: Vec<_> = state.regions.iter().map(|r| gossip_region_row(r)).collect();
    Table::new(rows).header(header).widths(&[
        Constraint::Max(120),
        Constraint::Min(4),
        Constraint::Min(5),
        // Constraint::Percentage(40),
    ])
}

fn gossip_region_row(region: &Region) -> Row<'static> {
    let cells = [
        format!("{:?}", region.coords),
        format!("{}", region.data.count.human_count_bare()),
        format!("{}", region.data.size.human_count_bytes()),
        // format!("{:?}", region.data.hash),
    ];
    Row::new(cells)
}
