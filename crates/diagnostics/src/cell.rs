use crate::display::human_size;
use fallible_iterator::FallibleIterator;
use holochain::{conductor::state::ConductorState, core::state::source_chain::SourceChain};
use holochain_state::{
    buffer::{BufKey, BufVal},
    db,
    env::EnvironmentWrite,
    prelude::*,
    typed::{Kv, UnitDbKey, UnitDbVal},
};
use holochain_types::{app::CellNick, cell::CellId};

pub async fn dump_cell_state(
    env: EnvironmentWrite,
    cell_id: CellId,
    cell_nick: &CellNick,
) -> anyhow::Result<()> {
    use db::*;
    let g = env.guard().await;
    let r = g.reader()?;

    macro_rules! dumper {
        ($db: ident) => {
            let db = Kv::new(env.get_db(&$db)?)?;
            dump_kv(db, &r, cell_nick)?;
        };
    }

    dumper!(ELEMENT_VAULT_PUBLIC_ENTRIES);
    dumper!(ELEMENT_VAULT_PRIVATE_ENTRIES);
    dumper!(ELEMENT_VAULT_HEADERS);
    dumper!(META_VAULT_LINKS);
    dumper!(META_VAULT_STATUS);
    dumper!(ELEMENT_CACHE_ENTRIES);
    dumper!(ELEMENT_CACHE_HEADERS);
    dumper!(CACHE_LINKS_META);
    dumper!(CACHE_STATUS_META);
    Ok(())
}

fn dump_kv(
    db: Kv<UnitDbKey, UnitDbVal>,
    reader: &Reader,
    cell_nick: &CellNick,
) -> anyhow::Result<()> {
    let count = db.iter(reader)?.count()?;
    println!("count: {}", count);
    Ok(())
}

// fn dump_source_chain(db: SourceChain, cell_nick: &CellNick) -> anyhow::Result<()> {
//     let header_count = db.
//     println!("+++++++ SourceChain for \"{}\" +++++++", cell_nick);
//     println!("Size: {}", human_size(bytes.len()));
//     println!("Data: {:#?}", state);

//     Ok(())
// }
