use kitsune_p2p::dht::hash::RegionHash;
use num_traits::Zero;
use rusqlite::{functions::*, *};

pub fn add_custom_functions(conn: &Connection) -> Result<()> {
    conn.create_aggregate_function(
        "REDUCE_XOR",
        -1,
        FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_DIRECTONLY,
        AggregateXor,
    )?;

    Ok(())
}

pub struct AggregateXor;

impl Aggregate<RegionHash, Vec<u8>> for AggregateXor {
    fn init(&self, _ctx: &mut Context<'_>) -> Result<RegionHash> {
        Ok(RegionHash::zero())
    }

    fn step(&self, ctx: &mut Context<'_>, v: &mut RegionHash) -> Result<()> {
        let blob: Vec<u8> = ctx.get(0)?;
        let len = blob.len();
        if let Some(a) = RegionHash::from_vec(blob) {
            v.xor(&a);
            Ok(())
        } else {
            Err(Error::UserFunctionError(
                format!(
                    "REDUCE_XOR can only handle BLOBs of 39 bytes, but encountered one of {} bytes",
                    len
                )
                .into(),
            ))
        }
    }

    fn finalize(&self, _ctx: &mut Context<'_>, v: Option<RegionHash>) -> Result<Vec<u8>> {
        Ok(v.unwrap_or_else(RegionHash::zero).0.to_vec())
    }
}
