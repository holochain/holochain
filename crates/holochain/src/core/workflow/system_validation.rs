use error::SysValidationResult;
use mockall::automock;

#[automock]
pub(crate) trait SystemValidation {
    fn check_entry_hash() -> SysValidationResult<()> {
        todo!()
    }
}

pub mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum SysValidationError {
        #[error("ValidationFailed")]
        ValidationFailed,
    }

    pub type SysValidationResult<T> = Result<T, SysValidationError>;
}
