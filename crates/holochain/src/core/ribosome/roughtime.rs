pub mod client;
pub mod known_servers;

use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::random_bytes::csprng_bytes;
use crate::core::ribosome::roughtime::client::*;
use crate::core::ribosome::roughtime::known_servers::servers;
use crate::core::ribosome::roughtime::known_servers::Server;
use roughenough::RtMessage;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::sync::Arc;
use sx_zome_types::RoughtimeInput;
use sx_zome_types::RoughtimeOutput;

const MIN_BLIND_LEN: usize = 64;
const DESIRED_CHAIN_LEN: usize = 3;
const MAX_ATTEMPTS: u8 = 3;

pub struct Nonce {
    bytes: [u8; 64],
    blind: Vec<u8>,
    encoded_message: Vec<u8>,
}

impl std::fmt::Debug for Nonce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Nonce").field("bytes", &self.bytes.to_vec()).field("blind", &self.blind).field("encoded_message", &self.encoded_message).finish()
    }
}

#[derive(Debug)]
pub struct ChainItem {
    nonce: Nonce,
    server: Server,
    server_response: RtMessage,
}

impl ChainItem {
    pub fn nonce(&self) -> &Nonce {
        &self.nonce
    }

    pub fn server_response(&self) -> &RtMessage {
        &self.server_response
    }

    pub fn server(&self) -> &Server {
        &self.server
    }
}

/// https://roughtime.googlesource.com/roughtime/+/HEAD/ECOSYSTEM.md#chaining-requests
impl Nonce {
    pub fn bytes(&self) -> &[u8; 64] {
        &self.bytes
    }

    pub fn generate(
        blind: &[u8],
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
            bytes.extend(blind);
            bytes
        } else {
            blind.to_vec()
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

fn try_chain(
    mut servers: Vec<Server>,
    nonce: Nonce,
    mut chain: Vec<ChainItem>,
) -> Result<Vec<ChainItem>, ()> {
    if chain.len() == DESIRED_CHAIN_LEN {
        Ok(chain)
    } else {
        match servers.pop() {
            // not enough servers!
            None => Err(()),
            Some(server) => {
                let addr = server
                    .addr()
                    .to_socket_addrs()
                    .unwrap()
                    .next()
                    .unwrap();

                let mut socket = UdpSocket::bind("0.0.0.0:0").expect("Couldn't open UDP socket");
                let request = make_request(nonce.bytes());

                socket.send_to(&request, addr).unwrap();

                let resp: RtMessage = receive_response(&mut socket);
                let next_nonce = match Nonce::generate(nonce.bytes(), Some(resp.clone())) {
                    Ok(v) => v,
                    // failed to generate a nonce!
                    Err(_) => return Err(()),
                };

                let response: ParsedResponse = ResponseHandler::new(
                    Some(server.pub_key().to_vec()),
                    resp.clone(),
                    *nonce.bytes(),
                )
                .extract_time();

                // the response does not verify according to roughtime protocol
                if !response.verified() {
                    return Err(());
                }

                chain.push(ChainItem {
                    nonce: nonce,
                    server,
                    server_response: resp,
                });

                // recurse
                try_chain(servers, next_nonce, chain)
            }
        }
    }
}

pub fn roughtime(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: RoughtimeInput,
) -> RoughtimeOutput {
    let mut attempt = 0;

    let chain: Vec<ChainItem> = loop {
        if attempt == MAX_ATTEMPTS {
            break vec![];
        } else {
            // TODO deal with unwrap
            match try_chain(
                servers(),
                Nonce::generate(input.inner_ref(), None).unwrap(),
                vec![],
            ) {
                Ok(chain) => break chain,
                Err(_) => {
                    attempt += 1;
                    continue;
                }
            }
        }
    };

    dbg!(chain);

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
