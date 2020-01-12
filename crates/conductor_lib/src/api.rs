
use skunkworx_core::types::ZomeInvocation;
use crate::conductor::CellHandle;

/// The set of messages that a conductor understands how to handle
pub enum ConductorApi {
    Admin(Admin),
    Crypto(Crypto),
    Test(Test),
    ZomeInvocation(CellHandle, ZomeInvocation),
}

pub enum Admin {}

pub enum Crypto {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

pub enum Test {
    AddAgent(AddAgentArgs)
}

pub struct AddAgentArgs {
    id: String,
    name: String,
}