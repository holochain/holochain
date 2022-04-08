use crate::prelude::*;

pub use holochain_integrity_types::x_salsa20_poly1305::*;
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct XSalsa20Poly1305Encrypt {
    key_ref: crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef,
    data: crate::x_salsa20_poly1305::data::XSalsa20Poly1305Data,
}

impl XSalsa20Poly1305Encrypt {
    pub fn new(
        key_ref: crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef,
        data: crate::x_salsa20_poly1305::data::XSalsa20Poly1305Data,
    ) -> Self {
        Self { key_ref, data }
    }

    pub fn as_key_ref_ref(&self) -> &crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef {
        &self.key_ref
    }

    pub fn as_data_ref(&self) -> &crate::x_salsa20_poly1305::data::XSalsa20Poly1305Data {
        &self.data
    }
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct X25519XSalsa20Poly1305Encrypt {
    sender: X25519PubKey,
    recipient: X25519PubKey,
    data: crate::x_salsa20_poly1305::data::XSalsa20Poly1305Data,
}

impl X25519XSalsa20Poly1305Encrypt {
    pub fn new(
        sender: X25519PubKey,
        recipient: X25519PubKey,
        data: crate::x_salsa20_poly1305::data::XSalsa20Poly1305Data,
    ) -> Self {
        Self {
            sender,
            recipient,
            data,
        }
    }

    pub fn as_sender_ref(&self) -> &X25519PubKey {
        &self.sender
    }

    pub fn as_recipient_ref(&self) -> &X25519PubKey {
        &self.recipient
    }

    pub fn as_data_ref(&self) -> &XSalsa20Poly1305Data {
        &self.data
    }
}
