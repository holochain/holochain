use crate::display::dump_kv;
use holochain_lmdb::{db, env::EnvironmentWrite, prelude::*};

pub async fn dump_wasm_state(env: EnvironmentWrite) -> anyhow::Result<()> {
    use db::*;
    let g = env.guard();
    let r = g.reader()?;

    dump_kv(&r, "wasm", env.get_db(&WASM)?)?;
    dump_kv(&r, "dna defs", env.get_db(&DNA_DEF)?)?;
    dump_kv(&r, "entry defs", env.get_db(&ENTRY_DEF)?)?;

    Ok(())
}
