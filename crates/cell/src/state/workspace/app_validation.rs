use super::Workspace;

use crate::state::workspace::WorkspaceResult;
use sx_state::{db::DbManager, error::{DatabaseError, DatabaseResult}, prelude::*};

pub struct AppValidationWorkspace {}

impl<'env> AppValidationWorkspace {
    pub fn new(_reader: Reader<'env>, _dbs: &DbManager) -> DatabaseResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for AppValidationWorkspace {
    fn commit_txn(self, writer: Writer) -> WorkspaceResult<()> {
        writer.commit().map_err(DatabaseError::from)?;
        Ok(())
    }
}
