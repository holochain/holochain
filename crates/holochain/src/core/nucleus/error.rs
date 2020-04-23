//! Errors that can occur while running [ZomeApi] functions

#![allow(missing_docs)]

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZomeApiError {}

pub type ZomeApiResult<T> = Result<T, ZomeApiError>;
