//! rust doesn't provide a fast binary copy function

use crate::*;

pub(crate) fn safe_mem_copy(dst: &mut [u8], dst_offset: usize, src: &[u8]) -> CryptoResult<()> {
    if dst_offset + src.len() > dst.len() {
        return Err(CryptoError::WriteOverflow);
    }

    // INVARIANTS:
    //   - we cannot write beyond the end of src - ensured by check above
    unsafe {
        std::ptr::copy(src.as_ptr(), dst.as_mut_ptr().add(dst_offset), src.len());
    }

    Ok(())
}
