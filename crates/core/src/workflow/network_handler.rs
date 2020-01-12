use crate::{cell::Cell, shims::call_zome_function, types::ZomeInvocation};
use crossbeam_channel::Sender;
use futures::never::Never;
use lib3h_protocol::{protocol_client::Lib3hClientProtocol, protocol_server::Lib3hServerProtocol};

pub async fn network_handler(
    msg: Lib3hServerProtocol,
    net_tx: Sender<Lib3hClientProtocol>,
) -> Result<(), Never> {
    match msg {
        _ => unimplemented!(),
    }
}
