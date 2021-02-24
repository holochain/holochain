use crate::display::human_size;
use crate::holochain::conductor::{state::ConductorState, ConductorStateDb};
use holochain_sqlite::{db::CONDUCTOR_STATE, env::DbWrite, prelude::*};

pub async fn dump_conductor_state(env: DbWrite) -> anyhow::Result<ConductorState> {
    let g = env.guard();
    let r = g.reader()?;
    let db = ConductorStateDb::new(env.get_table(&CONDUCTOR_STATE)?);
    let bytes = db.get_bytes(&r, &().into())?.unwrap();
    let state = db.get(&r, &().into())?.unwrap();

    println!("Size: {}", human_size(bytes.len()));
    println!("Data: {:#?}", state);
    Ok(state)
}
