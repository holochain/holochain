use super::error::MrBundleResult;
use std::io::Read;
use std::io::Write;

pub fn encode<T: serde::ser::Serialize>(data: &T) -> MrBundleResult<Vec<u8>> {
    let bytes = rmp_serde::to_vec_named(data)?;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&bytes)?;
    Ok(enc.finish()?)
}

pub fn decode<T: serde::de::DeserializeOwned>(compressed: &[u8]) -> MrBundleResult<T> {
    let mut gz = flate2::read::GzDecoder::new(&compressed[..]);
    let mut bytes = Vec::new();
    gz.read_to_end(&mut bytes)?;
    Ok(rmp_serde::from_read_ref(&bytes)?)
}
