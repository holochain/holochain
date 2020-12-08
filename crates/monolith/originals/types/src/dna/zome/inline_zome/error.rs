#![allow(missing_docs)]

use crate::holochain_zome_types::zome::FunctionName;
use thiserror::Error;

pub type InlineZomeResult<T> = Result<T, InlineZomeError>;

#[derive(Error, Debug)]
pub enum InlineZomeError {
    #[error("No such InlineZome callback: {0}")]
    NoSuchCallback(FunctionName),

    #[error("TODO: remove after crate re-org [B-03640]")]
    Infallible(#[from] std::convert::Infallible),
}
