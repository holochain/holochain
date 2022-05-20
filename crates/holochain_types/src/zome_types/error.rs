use holochain_zome_types::ZomeName;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZomeTypesError {
    #[error("There is more then the maximum of 255 Zomes in in a single DNA")]
    ZomeIndexOverflow,
    #[error("There is more then the maximum of 255 Entry Types in a single DNA")]
    EntryTypeIndexOverflow,
    #[error("There is more then the maximum of 255 Link Types in a single DNA")]
    LinkTypeIndexOverflow,
    #[error("Missing dependencies for zome {0}")]
    MissingDependenciesForZome(ZomeName),
}

pub type ZomeTypesResult<T> = Result<T, ZomeTypesError>;
