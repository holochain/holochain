/// Get a hex string representation of two chars per byte
pub fn bytes_to_hex(bytes: &[u8], caps: bool) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() + 2);
    if caps {
        for b in bytes {
            write!(&mut s, "{:02X}", b).ok();
        }
    } else {
        for b in bytes {
            write!(&mut s, "{:02x}", b).ok();
        }
    }
    s
}

/// Helpful pattern for debug formatting many bytes.
/// If the size is > 32 bytes, only the first 8 and last 8 bytes will be displayed.
pub fn many_bytes_string(bytes: &[u8]) -> String {
    if bytes.len() <= 32 {
        format!("0x{}", bytes_to_hex(bytes, false))
    } else {
        let l = bytes.len();
        format!(
            "[0x{}..{}; len={}]",
            bytes_to_hex(&bytes[0..8], false),
            bytes_to_hex(&bytes[l - 8..l], false),
            l
        )
    }
}
