use crate::prelude::{DatabaseError, DatabaseResult};
use holochain_zome_types::prelude::*;
use std::{
    num::TryFromIntError,
    time::{Duration, SystemTime},
};

pub fn time_to_micros(t: SystemTime) -> DatabaseResult<i64> {
    t.duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| DatabaseError::Other(e.into()))?
        .as_micros()
        .try_into()
        .map_err(|e: TryFromIntError| DatabaseError::Other(e.into()))
}

pub fn time_from_micros(micros: i64) -> DatabaseResult<SystemTime> {
    std::time::UNIX_EPOCH
        .checked_add(Duration::from_micros(micros as u64))
        .ok_or_else(|| {
            DatabaseError::Other(anyhow::anyhow!(
                "Got invalid i64 microsecond timestamp: {}",
                micros
            ))
        })
}
