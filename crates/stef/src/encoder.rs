use serde::{de::DeserializeOwned, Serialize};

pub trait Encoder<T: Serialize + DeserializeOwned>: Default + Send + Sync + 'static {
    fn encode(&self, action: &T) -> anyhow::Result<Vec<u8>>;
    fn decode(&self, encoded: &[u8]) -> anyhow::Result<T>;
}

#[derive(Default, Clone)]
pub struct RmpEncoder;

impl<T: Serialize + DeserializeOwned> Encoder<T> for RmpEncoder {
    fn encode(&self, action: &T) -> anyhow::Result<Vec<u8>> {
        Ok(rmp_serde::to_vec(action)?)
    }

    fn decode(&self, encoded: &[u8]) -> anyhow::Result<T> {
        Ok(rmp_serde::from_slice(encoded)?)
    }
}

#[derive(Default, Clone)]
pub struct JsonEncoder {
    pub pretty: bool,
}

impl<T: Serialize + DeserializeOwned> Encoder<T> for JsonEncoder {
    fn encode(&self, action: &T) -> anyhow::Result<Vec<u8>> {
        Ok(if self.pretty {
            serde_json::to_vec_pretty(action)?
        } else {
            serde_json::to_vec(action)?
        })
    }

    fn decode(&self, encoded: &[u8]) -> anyhow::Result<T> {
        Ok(serde_json::from_slice(encoded)?)
    }
}

/*
pub trait Encoder<T, R, W = R>
where
    T: Serialize + DeserializeOwned,
    R: std::io::Read,
    W: std::io::Write,
{
    fn encode(w: &mut W, action: &T) -> anyhow::Result<()>;
    fn decode(r: &R, encoded: &[u8]) -> anyhow::Result<T>;
}

pub struct RmpEncoder;

impl<T, R, W> Encoder<T, R, W> for RmpEncoder
where
    T: Serialize + DeserializeOwned,
    R: std::io::Read,
    W: std::io::Write,
{
    fn encode(w: &mut W, action: &T) -> anyhow::Result<()> {
        Ok(rmp_serde::encode::write(w, action)?)
    }

    fn decode(r: &R, encoded: &[u8]) -> anyhow::Result<T> {
        Ok(rmp_serde::from_read(r)?)
    }
}

pub struct JsonEncoder;

impl<T, R, W> Encoder<T, R, W> for JsonEncoder {
    fn encode(w: &mut W, action: &T) -> anyhow::Result<()> {
        Ok(serde_json::to_writer(w, action)?)
    }

    fn decode(r: &R, encoded: &[u8]) -> anyhow::Result<T> {
        Ok(serde_json::from_reader(r)?)
    }
}
*/
