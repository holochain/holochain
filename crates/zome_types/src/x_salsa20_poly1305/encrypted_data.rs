use crate::x_salsa20_poly1305::nonce::XSalsa20Poly1305Nonce;

#[derive(PartialEq, serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct XSalsa20Poly1305EncryptedData {
    nonce: XSalsa20Poly1305Nonce,
    #[serde(with = "serde_bytes")]
    encrypted_data: Vec<u8>,
}

impl XSalsa20Poly1305EncryptedData {
    pub fn new(nonce: XSalsa20Poly1305Nonce, encrypted_data: Vec<u8>) -> Self {
        Self {
            nonce,
            encrypted_data,
        }
    }

    pub fn as_nonce_ref(&self) -> &XSalsa20Poly1305Nonce {
        &self.nonce
    }

    pub fn as_encrypted_data_ref(&self) -> &[u8] {
        &self.encrypted_data
    }
}
