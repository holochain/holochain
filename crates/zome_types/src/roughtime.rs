use holochain_serialized_bytes::prelude::*;
big_array! { Sha512Bytes; }

#[derive(Serialize, Deserialize)]
pub struct Nonce {
    #[serde(with = "Sha512Bytes")]
    pub bytes: [u8; 64],
    pub blind: Vec<u8>,
    pub encoded_message: Vec<u8>,
}

impl Nonce {
    pub fn bytes(&self) -> &[u8; 64] {
        &self.bytes
    }

    pub fn blind(&self) -> &[u8] {
        &self.blind
    }

    pub fn encoded_message(&self) -> &[u8] {
        &self.encoded_message
    }
}

impl std::fmt::Debug for Nonce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Nonce")
            .field("bytes", &self.bytes.to_vec())
            .field("blind", &self.blind)
            .field("encoded_message", &self.encoded_message)
            .finish()
    }
}

impl PartialEq for Nonce {
    fn eq(&self, other: &Self) -> bool {
        let self_bytes: &[u8] = &self.bytes;
        let other_bytes: &[u8] = &other.bytes;
        self_bytes == other_bytes
            && self.blind == other.blind
            && self.encoded_message == other.encoded_message
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ChainItem {
    pub nonce: Nonce,
    pub server: Server,
    pub server_response: Vec<u8>,
}

impl ChainItem {
    pub fn nonce(&self) -> &Nonce {
        &self.nonce
    }

    pub fn server_response(&self) -> &[u8] {
        &self.server_response
    }

    pub fn server(&self) -> &Server {
        &self.server
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Server {
    pub addr: String,
    pub pub_key: [u8; 32],
}

impl Server {
    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub fn pub_key(&self) -> &[u8; 32] {
        &self.pub_key
    }
}
