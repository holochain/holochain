use crate::display::dump_kv;
use holochain_state::{db, env::EnvironmentWrite, prelude::*};

pub async fn dump_wasm_state(env: EnvironmentWrite) -> anyhow::Result<()> {
    use db::*;
    let g = env.guard().await;
    let r = g.reader()?;

    dump_kv(&r, "wasm", env.get_db(&WASM)?)?;
    dump_kv(&r, "dna defs", env.get_db(&DNA_DEF)?)?;
    dump_kv(&r, "entry defs", env.get_db(&ENTRY_DEF)?)?;

    Ok(())
}

/*
pub static ref WASM: DbKey<SingleStore> = DbKey::new(DbName::Wasm);
/// The key to access the DnaDef database
pub static ref DNA_DEF: DbKey<SingleStore> = DbKey::new(DbName::DnaDef);
/// The key to access the EntryDef database
pub static ref ENTRY_DEF: DbKey<SingleStore> = DbKey::new(DbName::EntryDef);
/// The key to access the AuthoredDhtOps database
*/
