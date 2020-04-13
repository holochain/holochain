pub mod client;
pub mod known_servers;

use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::random_bytes::csprng_bytes;
use crate::core::ribosome::roughtime::client::*;
use crate::core::ribosome::roughtime::known_servers::servers;
use crate::core::ribosome::RibosomeError;
use roughenough::RtMessage;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::sync::Arc;
use sx_zome_types::roughtime::ChainItem;
use sx_zome_types::roughtime::Nonce;
use sx_zome_types::roughtime::Server;
use sx_zome_types::RoughtimeInput;
use sx_zome_types::RoughtimeOutput;

const MIN_BLIND_LEN: usize = 64;
const DESIRED_CHAIN_LEN: usize = 3;
const MAX_ATTEMPTS: u8 = 3;

pub enum RoughTimeError {
    NotEnoughServers,
    NoSocketAddrs(String),
    Io(std::io::Error),
    Roughenough(roughenough::Error),
    Validation(Validation),
    Csprng(ring::error::Unspecified),
}

impl From<ring::error::Unspecified> for RoughTimeError {
    fn from(error: ring::error::Unspecified) -> RoughTimeError {
        RoughTimeError::Csprng(error)
    }
}

impl From<std::io::Error> for RoughTimeError {
    fn from(error: std::io::Error) -> RoughTimeError {
        RoughTimeError::Io(error)
    }
}

impl From<roughenough::Error> for RoughTimeError {
    fn from(error: roughenough::Error) -> RoughTimeError {
        RoughTimeError::Roughenough(error)
    }
}

/// https://roughtime.googlesource.com/roughtime/+/HEAD/ECOSYSTEM.md#chaining-requests
pub fn generate_nonce(
    blind: &[u8],
    last_message: Option<RtMessage>,
) -> Result<Nonce, RoughTimeError> {
    let mut nonce = Nonce {
        bytes: [0_u8; 64],
        blind: Vec::with_capacity(MIN_BLIND_LEN),
        encoded_message: Vec::new(),
    };

    // pad the blind out if it's too short with some random bytes
    nonce.blind = if blind.len() < MIN_BLIND_LEN {
        let mut bytes: Vec<u8> = csprng_bytes(MIN_BLIND_LEN - blind.len())?;
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

fn try_chain(
    mut servers: Vec<Server>,
    nonce: Nonce,
    mut chain: Vec<ChainItem>,
) -> Result<Vec<ChainItem>, RoughTimeError> {
    if chain.len() == DESIRED_CHAIN_LEN {
        Ok(chain)
    } else {
        match servers.pop() {
            None => Err(RoughTimeError::NotEnoughServers),
            Some(server) => {
                let addr = match server.addr().to_socket_addrs()?.next() {
                    Some(v) => v,
                    None => return Err(RoughTimeError::NoSocketAddrs(server.addr().to_string())),
                };

                let mut socket = UdpSocket::bind("0.0.0.0:0").expect("Couldn't open UDP socket");
                let request = make_request(nonce.bytes())?;

                socket.send_to(&request, addr)?;

                let resp: RtMessage = receive_response(&mut socket)?;
                // an empty blind will be filled with crypto random bytes
                let next_nonce = generate_nonce(&[], Some(resp.clone()))?;

                match ResponseHandler::new(server.pub_key().to_vec(), resp.clone(), *nonce.bytes())?
                    .validate()
                {
                    Validation::Valid => {}
                    v => return Err(RoughTimeError::Validation(v)),
                };

                chain.push(ChainItem {
                    nonce: nonce,
                    server,
                    server_response: resp.encode()?,
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
) -> Result<RoughtimeOutput, RibosomeError> {
    let mut attempt = 0;

    let chain: Vec<ChainItem> = loop {
        if attempt == MAX_ATTEMPTS {
            break vec![];
        } else {
            match try_chain(servers(), generate_nonce(input.inner_ref(), None)?, vec![]) {
                Ok(chain) => break chain,
                Err(_) => {
                    attempt += 1;
                    continue;
                }
            }
        }
    };

    Ok(RoughtimeOutput::new(chain))
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
