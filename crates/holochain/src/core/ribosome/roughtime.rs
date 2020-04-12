pub mod client;
pub mod known_servers;

use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::random_bytes::csprng_bytes;
use crate::core::ribosome::roughtime::client::*;
use crate::core::ribosome::roughtime::known_servers::servers;
use byteorder::LittleEndian;
use byteorder::ReadBytesExt;
use chrono::offset::TimeZone;
use chrono::Utc;
use roughenough::Tag;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::sync::Arc;
use sx_zome_types::RoughtimeInput;
use sx_zome_types::RoughtimeOutput;

struct Nonce([u8; 64]);

impl From<&RoughtimeInput> for Nonce {
    fn from(input: &RoughtimeInput) -> Nonce {
        let mut inner: [u8; 64] = [0; 64];
        match input.inner_ref().len() {
            l if l >= 64 => inner.copy_from_slice(&input.inner_ref()[0..64]),
            l if l < 64 => {
                let mut bytes: Vec<u8> = csprng_bytes(64 - l);
                bytes.splice(0..0, input.inner_ref().iter().cloned());
                inner.copy_from_slice(&bytes[0..64]);
            }
            _ => unreachable!(),
        }
        Nonce(inner)
    }
}

pub fn roughtime(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: RoughtimeInput,
) -> RoughtimeOutput {
    let num_requests = 3;

    for server in servers() {
        let addr = server.addr().to_socket_addrs().unwrap().next().unwrap();

        let mut requests = Vec::with_capacity(num_requests);
        for _ in 0..num_requests {
            let nonce = Nonce::from(&input);
            let socket = UdpSocket::bind("0.0.0.0:0").expect("Couldn't open UDP socket");
            let request = make_request(&nonce.0);
            requests.push((nonce, request, socket));
        }

        for &mut (_, ref request, ref mut socket) in &mut requests {
            socket.send_to(request, addr).unwrap();
        }

        for (nonce, _, mut socket) in requests {
            let resp = receive_response(&mut socket);

            let response =
                ResponseHandler::new(Some(server.pub_key().to_vec()), resp.clone(), nonce.0)
                    .extract_time();

            dbg!(
                "x: {} {} {}",
                response.verified(),
                response.midpoint(),
                response.radius()
            );

            let map = resp.into_hash_map();
            let _index = map[&Tag::INDX]
                .as_slice()
                .read_u32::<LittleEndian>()
                .unwrap();

            let seconds = response.midpoint() / 10_u64.pow(6);
            let nsecs = (response.midpoint() - (seconds * 10_u64.pow(6))) * 10_u64.pow(3);

            let ts = Utc.timestamp(seconds as i64, nsecs as u32);
            dbg!("y: {:?}", ts);
        }
    }

    RoughtimeOutput::new(())
}

#[cfg(test)]
pub mod wasm_test {
    use sx_zome_types::RoughtimeInput;
    use sx_zome_types::RoughtimeOutput;

    #[test]
    fn invoke_import_roughtime_test() {
        let _: RoughtimeOutput =
            crate::call_test_ribosome!("imports", "roughtime", RoughtimeInput::new(vec![1, 2, 3]));
    }
}
