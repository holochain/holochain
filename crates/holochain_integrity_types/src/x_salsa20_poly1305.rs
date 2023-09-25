use crate::prelude::*;
pub mod data;
pub mod encrypted_data;
pub mod key_ref;
pub mod nonce;
pub mod x25519;
use holochain_serialized_bytes::prelude::*;

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct XSalsa20Poly1305Decrypt {
    pub key_ref: crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef,
    pub encrypted_data: crate::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData,
}

impl XSalsa20Poly1305Decrypt {
    pub fn new(
        key_ref: crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef,
        encrypted_data: crate::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData,
    ) -> Self {
        Self {
            key_ref,
            encrypted_data,
        }
    }

    pub fn as_key_ref_ref(&self) -> &crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef {
        &self.key_ref
    }

    pub fn as_encrypted_data_ref(
        &self,
    ) -> &crate::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData {
        &self.encrypted_data
    }
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct X25519XSalsa20Poly1305Decrypt {
    pub recipient: X25519PubKey,
    pub sender: X25519PubKey,
    pub encrypted_data: XSalsa20Poly1305EncryptedData,
}

impl X25519XSalsa20Poly1305Decrypt {
    pub fn new(
        recipient: X25519PubKey,
        sender: X25519PubKey,
        encrypted_data: XSalsa20Poly1305EncryptedData,
    ) -> Self {
        Self {
            recipient,
            sender,
            encrypted_data,
        }
    }

    pub fn as_sender_ref(&self) -> &X25519PubKey {
        &self.sender
    }

    pub fn as_recipient_ref(&self) -> &X25519PubKey {
        &self.recipient
    }

    pub fn as_encrypted_data_ref(&self) -> &XSalsa20Poly1305EncryptedData {
        &self.encrypted_data
    }
}
