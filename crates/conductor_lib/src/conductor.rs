use crate::{
    api,
    error::{ConductorError, ConductorResult},
};
use crossbeam_channel::Receiver;
use skunkworx_core::{
    cell::{Cell, CellId},
    types::ZomeInvocation,
};
use std::collections::HashMap;

/// A conductor-specific name for a Cell
/// (Used to be instance_id)
pub type CellHandle = String;

pub struct Conductor {
    cells: HashMap<CellHandle, Cell>,
}

impl Conductor {
    pub async fn handle_api_message(&mut self, msg: api::ConductorApi) -> ConductorResult<()> {
        match msg {
            api::ConductorApi::ZomeInvocation(handle, invocation) => {
                let cell = self
                    .cells
                    .get(&handle)
                    .ok_or_else(|| ConductorError::NoSuchCell(handle))?;
                cell.invoke_zome(invocation);
                Ok(())
            }
            api::ConductorApi::Admin(msg) => match msg { },
            api::ConductorApi::Crypto(msg) => match msg {
                api::Crypto::Sign(payload) => unimplemented!(),
                api::Crypto::Encrypt(payload) => unimplemented!(),
                api::Crypto::Decrypt(payload) => unimplemented!(),
            },
            api::ConductorApi::Test(msg) => match msg {
                api::Test::AddAgent(args) => unimplemented!()
            },
        }
    }
}
