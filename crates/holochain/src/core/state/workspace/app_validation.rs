use super::Workspace;

use crate::core::state::workspace::WorkspaceResult;
use holochain_state::{db::DbManager, error::DatabaseResult, prelude::*};

pub struct AppValidationWorkspace {}

impl<'env> AppValidationWorkspace {
    pub fn new(_reader: Reader<'env>, _dbs: &DbManager) -> DatabaseResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for AppValidationWorkspace {
    fn commit_txn(self, writer: Writer) -> WorkspaceResult<()> {
        writer.commit()?;
        Ok(())
    }
}
