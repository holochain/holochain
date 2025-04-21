use super::error::MrBundleResult;
use crate::error::MrBundleError;
use std::io::Read;
use std::io::Write;

/// Get the compressed bytes for a bundle.
pub fn pack<T: serde::ser::Serialize>(data: &T) -> MrBundleResult<bytes::Bytes> {
    let bytes = rmp_serde::to_vec_named(data)?;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&bytes)
        .map_err(|e| MrBundleError::IoError("Failed to compress bundle".to_string(), e))?;
    Ok(enc
        .finish()
        .map_err(|e| MrBundleError::IoError("Failed to finish compressing bundle".to_string(), e))?
        .into())
}

/// Decompress and deserialize a bundle
///
/// This operation is the inverse of [`pack`].
pub fn unpack<T: serde::de::DeserializeOwned>(compressed: impl Read) -> MrBundleResult<T> {
    let mut gz = flate2::read::GzDecoder::new(compressed);
    let mut bytes = Vec::new();
    gz.read_to_end(&mut bytes)
        .map_err(|e| MrBundleError::IoError("Failed to decompress bundle".to_string(), e))?;
    rmp_serde::from_slice(&bytes)
        .map_err(|e| MrBundleError::MsgpackDecodeError(std::any::type_name::<T>().to_string(), e))
}
