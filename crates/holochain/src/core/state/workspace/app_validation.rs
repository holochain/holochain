use super::Workspace;

use crate::core::state::workspace::WorkspaceResult;
use holochain_state::{db::GetDb, error::DatabaseResult, prelude::*};

pub struct AppValidationWorkspace {}

impl<'env> AppValidationWorkspace {
    pub fn new(_reader: Reader<'env>, _dbs: &impl GetDb) -> DatabaseResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for AppValidationWorkspace {
    fn commit_txn(self, writer: Writer) -> WorkspaceResult<()> {
        writer.commit()?;
        Ok(())
    }
}
