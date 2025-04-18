use crate::error::MrBundleError;

use super::error::MrBundleResult;
use std::io::Read;
use std::io::Write;

/// Get compressed bytes from some serializable data
pub fn encode<T: serde::ser::Serialize>(data: &T) -> MrBundleResult<bytes::Bytes> {
    let bytes = rmp_serde::to_vec_named(data)?;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&bytes)?;
    Ok(enc.finish()?.into())
}

/// Decompress and deserialize some bytes (inverse of `encode`)
pub fn decode<T: serde::de::DeserializeOwned>(compressed: &[u8]) -> MrBundleResult<T> {
    let mut gz = flate2::read::GzDecoder::new(compressed);
    let mut bytes = Vec::new();
    gz.read_to_end(&mut bytes)?;
    rmp_serde::from_slice(&bytes)
        .map_err(|e| MrBundleError::MsgpackDecodeError(std::any::type_name::<T>().to_string(), e))
}
