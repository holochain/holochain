use error::SysValidationResult;
use holo_hash::HoloHash;
use mockall::automock;

#[automock]
pub trait SystemValidation {
    fn check_entry_hash(&self, _entry_hash: &HoloHash) -> SysValidationResult<()> {
        todo!()
    }
}

pub(crate) struct PlaceholderSysVal {}

impl SystemValidation for PlaceholderSysVal {}

pub mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum SysValidationError {
        #[error("ValidationFailed")]
        ValidationFailed,
    }

    pub type SysValidationResult<T> = Result<T, SysValidationError>;
}
