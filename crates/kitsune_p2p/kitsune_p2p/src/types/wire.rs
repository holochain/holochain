//! KitsuneP2p Wire Protocol Encoding Decoding

// The kitsune wire protocol is designed to be very light,
// both in terms of cpu overhead, and in terms of dependencies.

use crate::types::KitsuneP2pError;

/// The main kitsune wire message enum
#[derive(Debug)]
pub enum Wire {
    Call(Vec<u8>),
    Notify(Vec<u8>),
}

impl Wire {
    pub fn decode(data: Vec<u8>) -> Result<Self, KitsuneP2pError> {
        Wire::priv_decode(data)
    }

    pub fn encode(self) -> Vec<u8> {
        self.priv_encode()
    }

    pub fn call(payload: Vec<u8>) -> Self {
        Self::Call(payload)
    }

    pub fn notify(payload: Vec<u8>) -> Self {
        Self::Notify(payload)
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

/// a kitsune call message
const WIRE_CALL: u8 = 0x10;

/// a kitsune notify message
const WIRE_NOTIFY: u8 = 0x20;

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
            Wire::Call(payload) => Wire::priv_encode_inner(WIRE_CALL, payload),
            Wire::Notify(payload) => Wire::priv_encode_inner(WIRE_NOTIFY, payload),
        }
    }

    fn priv_decode(mut data: Vec<u8>) -> Result<Self, KitsuneP2pError> {
        match &data[..] {
            [KITSUNE_MAGIC_1, KITSUNE_MAGIC_2, KITSUNE_PROTO_VER, WIRE_CALL, ..] => {
                data.drain(..4);
                Ok(Wire::Call(data))
            }
            [KITSUNE_MAGIC_1, KITSUNE_MAGIC_2, KITSUNE_PROTO_VER, WIRE_NOTIFY, ..] => {
                data.drain(..4);
                Ok(Wire::Notify(data))
            }
            _ => Err(KitsuneP2pError::decoding_error(
                "invalid or corrupt kitsune p2p message".to_string(),
            )),
        }
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
            WIRE_CALL,
        ]);
        assert_matches!(res, Ok(Wire::Call(vec)) if vec.is_empty());
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
        let res = Wire::decode(vec![0xff, KITSUNE_MAGIC_2, KITSUNE_PROTO_VER, WIRE_CALL]);
        assert_matches!(res, Err(KitsuneP2pError::DecodingError(_)));
    }

    #[test]
    fn bad_decode_magic_2() {
        let res = Wire::decode(vec![KITSUNE_MAGIC_1, 0xff, KITSUNE_PROTO_VER, WIRE_CALL]);
        assert_matches!(res, Err(KitsuneP2pError::DecodingError(_)));
    }

    #[test]
    fn bad_decode_proto_ver() {
        let res = Wire::decode(vec![KITSUNE_MAGIC_1, KITSUNE_MAGIC_2, 0xff, WIRE_CALL]);
        assert_matches!(res, Err(KitsuneP2pError::DecodingError(_)));
    }
}
