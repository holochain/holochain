use crate::prelude::*;

pub use holochain_integrity_types::x_salsa20_poly1305::*;

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct XSalsa20Poly1305SharedSecretExport {
    sender: X25519PubKey,
    recipient: X25519PubKey,
    key_ref: crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef,
}

impl XSalsa20Poly1305SharedSecretExport {
    pub fn new(
        sender: X25519PubKey,
        recipient: X25519PubKey,
        key_ref: crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef,
    ) -> Self {
        Self {
            sender,
            recipient,
            key_ref,
        }
    }

    pub fn as_sender_ref(&self) -> &X25519PubKey {
        &self.sender
    }

    pub fn as_recipient_ref(&self) -> &X25519PubKey {
        &self.recipient
    }

    pub fn as_key_ref_ref(&self) -> &crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef {
        &self.key_ref
    }
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct XSalsa20Poly1305SharedSecretIngest {
    recipient: X25519PubKey,
    sender: X25519PubKey,
    encrypted_data: crate::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData,
    key_ref: Option<crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef>,
}

impl XSalsa20Poly1305SharedSecretIngest {
    pub fn new(
        recipient: X25519PubKey,
        sender: X25519PubKey,
        encrypted_data: crate::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData,
        key_ref: Option<crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef>,
    ) -> Self {
        Self {
            recipient,
            sender,
            encrypted_data,
            key_ref,
        }
    }

    pub fn as_recipient_ref(&self) -> &X25519PubKey {
        &self.recipient
    }

    pub fn as_sender_ref(&self) -> &X25519PubKey {
        &self.sender
    }

    pub fn as_encrypted_data_ref(
        &self,
    ) -> &crate::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData {
        &self.encrypted_data
    }

    pub fn as_key_ref_ref(
        &self,
    ) -> &Option<crate::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef> {
        &self.key_ref
    }
}

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
