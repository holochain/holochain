use crate::Header;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum DhtOpError {
    #[error(
        "Tried to create a DhtOp from a ChainElement that requires an Entry. Header type {0:?}"
    )]
    HeaderWithoutEntry(Header),
}

pub type DhtOpResult<T> = Result<T, DhtOpError>;
