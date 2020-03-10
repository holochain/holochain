use sx_types::{cell::CellHandle, nucleus::ZomeInvocation, shims::*};

// It's uncertain whether we'll actually use all of the following

/// The set of messages that a conductor understands how to handle
pub enum ConductorProtocol {
    Admin(AdminMethod),
    Crypto(Crypto),
    Network(Lib3hServerProtocol),
    Test(Test),
    ZomeInvocation(CellHandle, ZomeInvocation),
}

pub enum AdminMethod {
    Start(CellHandle),
    Stop(CellHandle),
}

pub enum Crypto {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

pub enum Test {
    AddAgent(AddAgentArgs),
}

#[allow(dead_code)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}
