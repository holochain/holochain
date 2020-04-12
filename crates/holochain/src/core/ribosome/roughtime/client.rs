// everything here adapted from roughtime client upstrea
// @see https://github.com/int08h/roughenough/blob/master/src/bin/roughenough-client.rs

use byteorder::{LittleEndian, ReadBytesExt};
use roughenough::merkle::root_from_paths;
use roughenough::sign::Verifier;
use roughenough::{RtMessage, Tag, CERTIFICATE_CONTEXT, SIGNED_RESPONSE_CONTEXT};
use std::collections::HashMap;
use std::net::UdpSocket;

pub fn make_request(nonce: &[u8]) -> Vec<u8> {
    let mut msg = RtMessage::new(1);
    msg.add_field(Tag::NONC, nonce).unwrap();
    msg.pad_to_kilobyte();

    msg.encode().unwrap()
}

pub fn receive_response(sock: &mut UdpSocket) -> RtMessage {
    let mut buf = [0; 744];
    let resp_len = sock.recv_from(&mut buf).unwrap().0;

    RtMessage::from_bytes(&buf[0..resp_len]).unwrap()
}

pub struct ResponseHandler {
    pub_key: Option<Vec<u8>>,
    msg: HashMap<Tag, Vec<u8>>,
    srep: HashMap<Tag, Vec<u8>>,
    cert: HashMap<Tag, Vec<u8>>,
    dele: HashMap<Tag, Vec<u8>>,
    nonce: [u8; 64],
}

pub struct ParsedResponse {
    verified: bool,
    midpoint: u64,
    radius: u32,
}

impl ParsedResponse {
    pub fn verified(&self) -> bool {
        self.verified
    }
    pub fn midpoint(&self) -> u64 {
        self.midpoint
    }
    pub fn radius(&self) -> u32 {
        self.radius
    }
}

impl ResponseHandler {
    pub fn new(pub_key: Option<Vec<u8>>, response: RtMessage, nonce: [u8; 64]) -> ResponseHandler {
        let msg = response.into_hash_map();
        let srep = RtMessage::from_bytes(&msg[&Tag::SREP])
            .unwrap()
            .into_hash_map();
        let cert = RtMessage::from_bytes(&msg[&Tag::CERT])
            .unwrap()
            .into_hash_map();
        let dele = RtMessage::from_bytes(&cert[&Tag::DELE])
            .unwrap()
            .into_hash_map();

        ResponseHandler {
            pub_key,
            msg,
            srep,
            cert,
            dele,
            nonce,
        }
    }

    pub fn extract_time(&self) -> ParsedResponse {
        let midpoint = self.srep[&Tag::MIDP]
            .as_slice()
            .read_u64::<LittleEndian>()
            .unwrap();
        let radius = self.srep[&Tag::RADI]
            .as_slice()
            .read_u32::<LittleEndian>()
            .unwrap();

        let verified = if self.pub_key.is_some() {
            self.validate_dele();
            self.validate_srep();
            self.validate_merkle();
            self.validate_midpoint(midpoint);
            true
        } else {
            false
        };

        ParsedResponse {
            verified,
            midpoint,
            radius,
        }
    }

    fn validate_dele(&self) {
        let mut full_cert = Vec::from(CERTIFICATE_CONTEXT.as_bytes());
        full_cert.extend(&self.cert[&Tag::DELE]);

        assert!(
            self.validate_sig(
                self.pub_key.as_ref().unwrap(),
                &self.cert[&Tag::SIG],
                &full_cert
            ),
            "Invalid signature on DELE tag, response may not be authentic"
        );
    }

    fn validate_srep(&self) {
        let mut full_srep = Vec::from(SIGNED_RESPONSE_CONTEXT.as_bytes());
        full_srep.extend(&self.msg[&Tag::SREP]);

        assert!(
            self.validate_sig(&self.dele[&Tag::PUBK], &self.msg[&Tag::SIG], &full_srep),
            "Invalid signature on SREP tag, response may not be authentic"
        );
    }

    fn validate_merkle(&self) {
        let srep = RtMessage::from_bytes(&self.msg[&Tag::SREP])
            .unwrap()
            .into_hash_map();
        let index = self.msg[&Tag::INDX]
            .as_slice()
            .read_u32::<LittleEndian>()
            .unwrap();
        let paths = &self.msg[&Tag::PATH];

        let hash = root_from_paths(index as usize, &self.nonce, paths);

        assert_eq!(
            hash,
            srep[&Tag::ROOT],
            "Nonce is not present in the response's merkle tree"
        );
    }

    fn validate_midpoint(&self, midpoint: u64) {
        let mint = self.dele[&Tag::MINT]
            .as_slice()
            .read_u64::<LittleEndian>()
            .unwrap();
        let maxt = self.dele[&Tag::MAXT]
            .as_slice()
            .read_u64::<LittleEndian>()
            .unwrap();

        assert!(
            midpoint >= mint,
            "Response midpoint {} lies *before* delegation span ({}, {})",
            midpoint,
            mint,
            maxt
        );
        assert!(
            midpoint <= maxt,
            "Response midpoint {} lies *after* delegation span ({}, {})",
            midpoint,
            mint,
            maxt
        );
    }

    fn validate_sig(&self, public_key: &[u8], sig: &[u8], data: &[u8]) -> bool {
        let mut verifier = Verifier::new(public_key);
        verifier.update(data);
        verifier.verify(sig)
    }
}
