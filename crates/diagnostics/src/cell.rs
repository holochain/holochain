use crate::display::{dump_kv, dump_kvi};
use holochain_sqlite::{db, env::DbWrite, prelude::*};
use holochain_types::{app::CellNick, cell::CellId};

pub async fn dump_cell_state(
    env: DbWrite,
    _cell_id: CellId,
    cell_nick: &CellNick,
) -> anyhow::Result<()> {
    use db::*;
    let g = env.guard();
    let r = g.reader()?;

    macro_rules! kv {
        ($name: expr, $db: ident) => {
            let db = env.get_table(&$db)?;
            dump_kv(&r, $name, db)?;
        };
    }

    macro_rules! kvi {
        ($name: expr, $db: ident) => {
            let db = env.get_table(&$db)?;
            dump_kvi(&r, $name, db)?;
        };
    }

    println!();
    println!(
        "+++++++++++++++++++++++++  cell \"{}\"  +++++++++++++++++++++++++",
        cell_nick
    );
    println!();

    kvi!("chain sequence", TableName::ChainSequence);
    kv!(
        "element vault - public entries",
        ELEMENT_VAULT_PUBLIC_ENTRIES
    );
    kv!(
        "element vault - private entries",
        ELEMENT_VAULT_PRIVATE_ENTRIES
    );
    kv!("element vault - headers", ELEMENT_VAULT_HEADERS);
    kv!("metadata vault - links", META_VAULT_LINKS);
    kv!("metadata vault - misc", META_VAULT_MISC);

    kv!("element cache - entries", TableName::ElementCacheEntries);
    kv!("element cache - headers", TableName::ElementCacheHeaders);
    kv!("metadata cache - links", TableName::MetaCacheLinks);
    kv!("metadata cache - status", TableName::MetaCacheStatus);

    kv!("integration queue", INTEGRATION_LIMBO);
    kv!("integrated dht ops", INTEGRATED_DHT_OPS);
    kv!("authored dht ops", AUTHORED_DHT_OPS);

    Ok(())
}
