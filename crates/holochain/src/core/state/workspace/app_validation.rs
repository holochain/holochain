// Remove this one implemented
#![allow(missing_docs)]

use super::Workspace;

use crate::core::state::workspace::WorkspaceResult;
use holochain_state::{error::DatabaseResult, prelude::*};

pub struct AppValidationWorkspace {}

impl<'env> AppValidationWorkspace {
    pub fn new(_reader: &'env Reader<'env>, _dbs: &impl GetDb) -> DatabaseResult<Self> {
        unimplemented!()
    }
    fn commit_txn(self, writer: Writer) -> WorkspaceResult<()> {
        writer.commit()?;
        Ok(())
    }
}
