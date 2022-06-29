use kitsune_p2p::dht::{
    hash::{hash_slice_32, Hash32},
    region::slice_xor,
};
use rusqlite::{functions::*, types::ValueRef, *};

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

impl Aggregate<Hash32, Vec<u8>> for AggregateXor {
    fn init(&self, _ctx: &mut Context<'_>) -> Result<Hash32> {
        Ok([0; 32])
    }

    fn step(&self, ctx: &mut Context<'_>, v: &mut Hash32) -> Result<()> {
        let blob: &[u8] = match ctx.get_raw(0) {
            ValueRef::Blob(b) => Ok(b),
            v => Err(rusqlite::Error::InvalidFunctionParameterType(
                0,
                v.data_type(),
            )),
        }?;
        let len = blob.len();
        if len == 39 {
            slice_xor(v, hash_slice_32(blob));
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

    fn finalize(&self, _ctx: &mut Context<'_>, v: Option<Hash32>) -> Result<Vec<u8>> {
        Ok(v.unwrap_or([0; 32]).to_vec())
    }
}
