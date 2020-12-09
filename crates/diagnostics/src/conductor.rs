use crate::display::human_size;
use crate::holochain::conductor::{state::ConductorState, ConductorStateDb};
use holochain_lmdb::{db::CONDUCTOR_STATE, env::EnvironmentWrite, prelude::*};

pub async fn dump_conductor_state(env: EnvironmentWrite) -> anyhow::Result<ConductorState> {
    let g = env.guard();
    let r = g.reader()?;
    let db = ConductorStateDb::new(env.get_db(&CONDUCTOR_STATE)?);
    let bytes = db.get_bytes(&r, &().into())?.unwrap();
    let state = db.get(&r, &().into())?.unwrap();

    println!("Size: {}", human_size(bytes.len()));
    println!("Data: {:#?}", state);
    Ok(state)
}
