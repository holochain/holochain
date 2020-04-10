pub mod client;
pub mod known_servers;

use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::RoughtimeInput;
use sx_zome_types::RoughtimeOutput;
use crate::core::ribosome::roughtime::client::*;
use crate::core::ribosome::roughtime::known_servers::servers;
use std::net::ToSocketAddrs;

pub fn roughtime(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: RoughtimeInput,
) -> RoughtimeOutput {

    let num_requests = 3;

    for server in servers() {
        let addr = server.addr().to_socket_addrs().unwrap().next().unwrap();

        let mut requests = Vec::with_capacity(num_requests);
        for _ in 0..num_requests {
            let nonce = create_nonce();
            let socket = UdpSocket::bind("0.0.0.0:0").expect("Couldn't open UDP socket");
            let request = make_request(&nonce);
            requests.push((nonce, request, socket));
        }

        for &mut (_, ref request, ref mut socket) in &mut requests {
            socket.send_to(request, addr).unwrap();
        }

        for (nonce, _, mut socket) in requests {
            let resp = receive_response(&mut socket);

            let ParsedResponse {
                verified,
                midpoint,
                radius,
            } = ResponseHandler::new(Some(server.pub_key().to_vec()), resp.clone(), nonce).extract_time();

            dbg!("x: {} {} {}", verified, midpoint, radius);

            let map = resp.into_hash_map();
            let _index = map[&Tag::INDX]
            .as_slice()
            .read_u32::<LittleEndian>()
            .unwrap();

            let seconds = midpoint / 10_u64.pow(6);
            let nsecs = (midpoint - (seconds * 10_u64.pow(6))) * 10_u64.pow(3);

            let ts = Utc.timestamp(seconds as i64, nsecs as u32);
            dbg!("y: {:?}", ts);
        }
    }
}

#[cfg(test)]
pub mod wasm_test {
    use sx_zome_types::zome_io::SysTimeOutput;
    use sx_zome_types::SysTimeInput;

    #[test]
    fn invoke_import_sys_time_test() {
        let _: SysTimeOutput =
            crate::call_test_ribosome!("imports", "sys_time", SysTimeInput::new(()));
    }
}
