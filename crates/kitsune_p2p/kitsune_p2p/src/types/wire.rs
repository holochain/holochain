//! KitsuneP2p Wire Protocol Encoding Decoding

// The kitsune wire protocol is designed to be very light,
// both in terms of cpu overhead, and in terms of dependencies.

use crate::types::KitsuneP2pError;

/// The main kitsune wire message enum
#[derive(Debug)]
pub enum Wire {
    Request(Vec<u8>),
    Broadcast(Vec<u8>),
}

impl Wire {
    pub fn decode(data: Vec<u8>) -> Result<Self, KitsuneP2pError> {
        Wire::priv_decode(data)
    }

    pub fn encode(self) -> Vec<u8> {
        self.priv_encode()
    }

    pub fn request(data: Vec<u8>) -> Self {
        Self::Request(data)
    }

    pub fn broadcast(data: Vec<u8>) -> Self {
        Self::Broadcast(data)
    }
}

// -- private -- //

/// protocol identification heuristic
const KITSUNE_MAGIC_1: u8 = 0xdb;

/// protocol identification heuristic
const KITSUNE_MAGIC_2: u8 = 0x55;

/// protocol version marker
const KITSUNE_PROTO_VER: u8 = 0x00;

// list of message type bytes

/// a kitsune request message
const WIRE_REQUEST: u8 = 0x10;

/// a kitsune broadcast message
const WIRE_BROADCAST: u8 = 0x20;

impl Wire {
    fn priv_encode_inner(msg_type: u8, mut msg: Vec<u8>) -> Vec<u8> {
        let mut out = Vec::with_capacity(msg.len() + 4);
        out.push(KITSUNE_MAGIC_1);
        out.push(KITSUNE_MAGIC_2);
        out.push(KITSUNE_PROTO_VER);
        out.push(msg_type);
        out.append(&mut msg);
        out
    }

    fn priv_encode(self) -> Vec<u8> {
        match self {
            Wire::Request(msg) => Wire::priv_encode_inner(WIRE_REQUEST, msg),
            Wire::Broadcast(msg) => Wire::priv_encode_inner(WIRE_BROADCAST, msg),
        }
    }

    fn priv_decode(mut data: Vec<u8>) -> Result<Self, KitsuneP2pError> {

        match data.get(..4) {
            Some([KITSUNE_MAGIC_1, KITSUNE_MAGIC_2, KITSUNE_PROTO_VER, WIRE_REQUEST]) => {
                data.drain(0..4);
                Ok(Wire::Request(data))
            }
            Some([KITSUNE_MAGIC_1, KITSUNE_MAGIC_2, KITSUNE_PROTO_VER, WIRE_BROADCAST]) => {
                data.drain(0..4);
                Ok(Wire::Broadcast(data))
            }
            _ => Err(KitsuneP2pError::decoding_error(
                "invalid or corrupt kitsune p2p message".to_string(),
            )),
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::*;

    #[test]
    fn ok_decode() {
        let res = Wire::decode(vec![
            KITSUNE_MAGIC_1,
            KITSUNE_MAGIC_2,
            KITSUNE_PROTO_VER,
            WIRE_REQUEST,
        ]);
        assert_matches!(res, Ok(Wire::Request(vec)) if vec.is_empty());
    }

    #[test]
    fn bad_decode_size() {
        let res = Wire::decode(vec![KITSUNE_MAGIC_1, KITSUNE_MAGIC_2, KITSUNE_PROTO_VER]);
        assert_matches!(res, Err(KitsuneP2pError::DecodingError(_)));
    }

    #[test]
    fn bad_decode_msg_type() {
        let res = Wire::decode(vec![
            KITSUNE_MAGIC_1,
            KITSUNE_MAGIC_2,
            KITSUNE_PROTO_VER,
            0xff,
        ]);
        assert_matches!(res, Err(KitsuneP2pError::DecodingError(_)));
    }

    #[test]
    fn bad_decode_magic_1() {
        let res = Wire::decode(vec![0xff, KITSUNE_MAGIC_2, KITSUNE_PROTO_VER, WIRE_REQUEST]);
        assert_matches!(res, Err(KitsuneP2pError::DecodingError(_)));
    }

    #[test]
    fn bad_decode_magic_2() {
        let res = Wire::decode(vec![KITSUNE_MAGIC_1, 0xff, KITSUNE_PROTO_VER, WIRE_REQUEST]);
        assert_matches!(res, Err(KitsuneP2pError::DecodingError(_)));
    }

    #[test]
    fn bad_decode_proto_ver() {
        let res = Wire::decode(vec![KITSUNE_MAGIC_1, KITSUNE_MAGIC_2, 0xff, WIRE_REQUEST]);
        assert_matches!(res, Err(KitsuneP2pError::DecodingError(_)));
    }
}
