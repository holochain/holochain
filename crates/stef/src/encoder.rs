use serde::{de::DeserializeOwned, Serialize};

pub trait Encoder: Default + Send + Sync + 'static {
    fn encode<T: Serialize>(&self, action: &T) -> anyhow::Result<Vec<u8>>;
    fn decode<T: DeserializeOwned>(&self, encoded: &[u8]) -> anyhow::Result<T>;
}

#[derive(Default, Clone)]
pub struct RmpEncoder;

impl Encoder for RmpEncoder {
    fn encode<T: Serialize>(&self, action: &T) -> anyhow::Result<Vec<u8>> {
        Ok(rmp_serde::to_vec(action)?)
    }

    fn decode<T: DeserializeOwned>(&self, encoded: &[u8]) -> anyhow::Result<T> {
        Ok(rmp_serde::from_slice(encoded)?)
    }
}

#[derive(Default, Clone)]
pub struct JsonEncoder {
    pub pretty: bool,
}

impl Encoder for JsonEncoder {
    fn encode<T: Serialize>(&self, action: &T) -> anyhow::Result<Vec<u8>> {
        Ok(if self.pretty {
            serde_json::to_vec_pretty(action)?
        } else {
            serde_json::to_vec(action)?
        })
    }

    fn decode<T: DeserializeOwned>(&self, encoded: &[u8]) -> anyhow::Result<T> {
        Ok(serde_json::from_slice(encoded)?)
    }
}
