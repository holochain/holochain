use sx_types::error::SkunkError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZomeApiError {

    #[error("generic error")]
    Generic(#[from] SkunkError)
}

pub type ZomeApiResult<T> = Result<T, ZomeApiError>;
