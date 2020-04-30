use super::Workspace;

use crate::core::state::workspace::WorkspaceResult;
use holochain_state::{db::DbManager, error::DatabaseResult, prelude::*};

pub struct AppValidationWorkspace {}


impl<'env> Workspace<'env> for AppValidationWorkspace {
    fn new(_reader: &'env Reader<'env>, _dbs: &DbManager) -> WorkspaceResult<Self> {
        unimplemented!()
    }
    fn commit_txn(self, writer: Writer) -> WorkspaceResult<()> {
        writer.commit()?;
        Ok(())
    }
}
