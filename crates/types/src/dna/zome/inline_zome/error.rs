#![allow(missing_docs)]

use thiserror::Error;

pub type InlineZomeResult<T> = Result<T, InlineZomeError>;

#[derive(Error, Debug)]
pub enum InlineZomeError {}
