//! KitsuneP2p Wire Protocol Encoding Decoding

// The kitsune wire protocol is designed to be very light,
// both in terms of cpu overhead, and in terms of dependencies.

/// The main kitsune wire message enum
pub enum Wire {
    Request(Vec<u8>),
    Broadcast(Vec<u8>),
}

impl Wire {
    pub fn decode(data: Vec<u8>) -> Result<Self, ()> {
        Wire::priv_decode(data)
    }

    pub fn encode(self) -> Vec<u8> {
        self.priv_encode()
    }

    pub fn request(data: Vec<u8>) -> Self {
        Self::Request(data)
    }

    #[allow(dead_code)]
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
        let mut out = vec![
            KITSUNE_MAGIC_1,
            KITSUNE_MAGIC_2,
            KITSUNE_PROTO_VER,
            msg_type,
        ];
        out.append(&mut msg);
        out
    }

    fn priv_encode(self) -> Vec<u8> {
        match self {
            Wire::Request(msg) => Wire::priv_encode_inner(WIRE_REQUEST, msg),
            Wire::Broadcast(msg) => Wire::priv_encode_inner(WIRE_BROADCAST, msg),
        }
    }

    fn priv_decode(mut data: Vec<u8>) -> Result<Self, ()> {
        if data.len() < 4
            || data[0] != KITSUNE_MAGIC_1
            || data[1] != KITSUNE_MAGIC_2
            || data[2] != KITSUNE_PROTO_VER
        {
            return Err(());
        }
        data.remove(0);
        data.remove(0);
        data.remove(0);
        match data.remove(0) {
            WIRE_REQUEST => Ok(Wire::Request(data)),
            WIRE_BROADCAST => Ok(Wire::Broadcast(data)),
            _ => Err(()),
        }
    }
}
