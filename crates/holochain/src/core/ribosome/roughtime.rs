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
use roughenough::RtMessage;
use roughenough::Tag;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::sync::Arc;
use sx_zome_types::RoughtimeInput;
use sx_zome_types::RoughtimeOutput;

const MIN_BLIND_LEN: usize = 64;

pub struct Nonce {
    bytes: [u8; 64],
    blind: Vec<u8>,
    encoded_message: Vec<u8>,
}

/// https://roughtime.googlesource.com/roughtime/+/HEAD/ECOSYSTEM.md#chaining-requests
impl Nonce {
    pub fn bytes(&self) -> &[u8; 64] {
        &self.bytes
    }

    pub fn generate(
        blind: Vec<u8>,
        last_message: Option<RtMessage>,
    ) -> Result<Nonce, roughenough::Error> {
        let mut nonce = Nonce {
            bytes: [0_u8; 64],
            blind: Vec::with_capacity(MIN_BLIND_LEN),
            encoded_message: Vec::new(),
        };

        // pad the blind out if it's too short with some random bytes
        nonce.blind = if blind.len() < MIN_BLIND_LEN {
            let mut bytes: Vec<u8> = csprng_bytes(MIN_BLIND_LEN - blind.len());
            bytes.extend(&blind);
            bytes
        } else {
            blind
        };
        assert!(nonce.blind.len() >= MIN_BLIND_LEN);

        nonce.bytes.copy_from_slice(&match last_message {
            // in the case that there is no previous message we can just SHA-512 the blind to enforce
            // blinding and bytes length
            None => ring::digest::digest(&ring::digest::SHA512, &nonce.blind)
                .as_ref()
                .to_owned(),
            // normal algorithm is SHA-512(SHA-512(previous-reply) + blind)
            Some(message) => {
                nonce.encoded_message = message.encode()?;
                let mut hashed_last_message: Vec<u8> =
                    ring::digest::digest(&ring::digest::SHA512, &nonce.encoded_message)
                        .as_ref()
                        .to_vec();
                hashed_last_message.extend(&nonce.blind);
                ring::digest::digest(&ring::digest::SHA512, &hashed_last_message)
                    .as_ref()
                    .to_owned()
            }
        });
        Ok(nonce)
    }
}

pub fn roughtime(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: RoughtimeInput,
) -> RoughtimeOutput {
    // TODO deal with unwrap
    let nonce = Nonce::generate(input.inner(), None).unwrap();
    dbg!(&nonce.bytes().to_vec());

    for server in servers()[0..3].iter() {
        let addr = server.addr().to_socket_addrs().unwrap().next().unwrap();


        let mut socket = UdpSocket::bind("0.0.0.0:0").expect("Couldn't open UDP socket");
        let request = make_request(nonce.bytes());

        socket.send_to(&request, addr).unwrap();

        let resp = receive_response(&mut socket);

        let response =
            ResponseHandler::new(Some(server.pub_key().to_vec()), resp.clone(), *nonce.bytes())
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
