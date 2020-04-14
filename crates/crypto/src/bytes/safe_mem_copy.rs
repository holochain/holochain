use crate::*;

pub(crate) fn safe_mem_copy(dst: &mut [u8], dst_offset: usize, src: &[u8]) -> CryptoResult<()> {
    let dst_subslice = dst
        .get_mut(dst_offset..dst_offset + src.len())
        .ok_or(CryptoError::WriteOverflow)?;
    dst_subslice.copy_from_slice(src);
    Ok(())
}
