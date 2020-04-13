// everything here adapted from roughtime client upstrea
// @see https://github.com/int08h/roughenough/blob/master/src/bin/roughenough-client.rs

use crate::core::ribosome::roughtime::RoughTimeError;
use byteorder::{LittleEndian, ReadBytesExt};
use roughenough::merkle::root_from_paths;
use roughenough::sign::Verifier;
use roughenough::{RtMessage, Tag, CERTIFICATE_CONTEXT, SIGNED_RESPONSE_CONTEXT};
use std::collections::HashMap;
use std::net::UdpSocket;

#[derive(PartialEq)]
pub enum Validation {
    Valid,
    TimeBelowMin,
    TimeAboveMax,
    InvalidSignatureDele,
    InvalidSignatureSrep,
    MerkleMissingNonce,
}

pub fn make_request(nonce: &[u8]) -> Result<Vec<u8>, RoughTimeError> {
    let mut msg = RtMessage::new(1);
    msg.add_field(Tag::NONC, nonce)?;
    msg.pad_to_kilobyte();

    Ok(msg.encode()?)
}

pub fn receive_response(sock: &mut UdpSocket) -> Result<RtMessage, RoughTimeError> {
    let mut buf = [0; 744];
    let resp_len = sock.recv_from(&mut buf)?.0;

    Ok(RtMessage::from_bytes(&buf[0..resp_len])?)
}

pub struct ResponseHandler {
    pub_key: Vec<u8>,
    msg: HashMap<Tag, Vec<u8>>,
    srep: HashMap<Tag, Vec<u8>>,
    cert: HashMap<Tag, Vec<u8>>,
    dele: HashMap<Tag, Vec<u8>>,
    nonce: [u8; 64],
}

impl ResponseHandler {
    pub fn new(
        pub_key: Vec<u8>,
        response: RtMessage,
        nonce: [u8; 64],
    ) -> Result<ResponseHandler, RoughTimeError> {
        let msg = response.into_hash_map();
        let srep = RtMessage::from_bytes(&msg[&Tag::SREP])?.into_hash_map();
        let cert = RtMessage::from_bytes(&msg[&Tag::CERT])?.into_hash_map();
        let dele = RtMessage::from_bytes(&cert[&Tag::DELE])?.into_hash_map();

        Ok(ResponseHandler {
            pub_key,
            msg,
            srep,
            cert,
            dele,
            nonce,
        })
    }

    fn validate_sig(&self, public_key: &[u8], sig: &[u8], data: &[u8]) -> bool {
        let mut verifier = Verifier::new(public_key);
        verifier.update(data);
        verifier.verify(sig)
    }

    fn validate_dele(&self) -> bool {
        let mut full_cert = Vec::from(CERTIFICATE_CONTEXT.as_bytes());
        full_cert.extend(&self.cert[&Tag::DELE]);

        self.validate_sig(&self.pub_key, &self.cert[&Tag::SIG], &full_cert)
    }

    fn validate_srep(&self) -> bool {
        let mut full_srep = Vec::from(SIGNED_RESPONSE_CONTEXT.as_bytes());
        full_srep.extend(&self.msg[&Tag::SREP]);

        self.validate_sig(&self.dele[&Tag::PUBK], &self.msg[&Tag::SIG], &full_srep)
    }

    fn parse_srep(&self) -> Result<HashMap<Tag, Vec<u8>>, RoughTimeError> {
        Ok(RtMessage::from_bytes(&self.msg[&Tag::SREP])?.into_hash_map())
    }

    fn parse_index(&self) -> Result<u32, RoughTimeError> {
        Ok(self.msg[&Tag::INDX].as_slice().read_u32::<LittleEndian>()?)
    }

    fn validate_merkle(&self) -> bool {
        match self.parse_srep() {
            Ok(srep) => match self.parse_index() {
                Ok(index) => {
                    let paths = &self.msg[&Tag::PATH];
                    let hash = root_from_paths(index as usize, &self.nonce, paths);
                    hash == srep[&Tag::ROOT]
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn validate_mint(&self) -> bool {
        match self.parse_midpoint() {
            Ok(midpoint) => match self.parse_mint() {
                Ok(mint) => midpoint >= mint,
                _ => false,
            },
            _ => false,
        }
    }

    fn validate_maxt(&self) -> bool {
        match self.parse_midpoint() {
            Ok(midpoint) => match self.parse_maxt() {
                Ok(maxt) => midpoint <= maxt,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn validate(&self) -> Validation {
        if !self.validate_mint() {
            return Validation::TimeBelowMin;
        }
        if !self.validate_maxt() {
            return Validation::TimeAboveMax;
        }
        if !self.validate_dele() {
            return Validation::InvalidSignatureDele;
        }
        if !self.validate_srep() {
            return Validation::InvalidSignatureSrep;
        }
        if !self.validate_merkle() {
            return Validation::MerkleMissingNonce;
        }
        Validation::Valid
    }

    pub fn parse_midpoint(&self) -> Result<u64, RoughTimeError> {
        Ok(self.srep[&Tag::MIDP]
            .as_slice()
            .read_u64::<LittleEndian>()?)
    }

    pub fn parse_radius(&self) -> Result<u32, RoughTimeError> {
        Ok(self.srep[&Tag::RADI]
            .as_slice()
            .read_u32::<LittleEndian>()?)
    }

    pub fn parse_mint(&self) -> Result<u64, RoughTimeError> {
        Ok(self.dele[&Tag::MINT]
            .as_slice()
            .read_u64::<LittleEndian>()?)
    }

    pub fn parse_maxt(&self) -> Result<u64, RoughTimeError> {
        Ok(self.dele[&Tag::MAXT]
            .as_slice()
            .read_u64::<LittleEndian>()?)
    }
}
