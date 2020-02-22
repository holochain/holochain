
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkspaceError {

}

pub type WorkspaceResult<T> = Result<T, WorkspaceError>;
