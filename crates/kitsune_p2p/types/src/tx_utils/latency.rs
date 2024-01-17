use once_cell::sync::Lazy;

/// this is a reference instance in time
/// all latency info is encoded relative to this
static LOC_EPOCH: Lazy<tokio::time::Instant> = Lazy::new(tokio::time::Instant::now);

/// this tag identifies that a latency marker will follow
/// as a little endian ieee f64, this will decode to NaN.
const LAT_TAG: &[u8; 8] = &[0xff, 0xff, 0xff, 0xfe, 0xfe, 0xff, 0xff, 0xff];

/// Fill a buffer with data that is readable as latency information.
/// Note, the minimum message size to get the timing data across is 16 bytes.
pub fn fill_with_latency_info(buf: &mut [u8]) {
    if buf.is_empty() {
        return;
    }

    // make sure we call this first, so we don't go back in time
    let epoch = *LOC_EPOCH;

    let now = tokio::time::Instant::now();
    let now = now.duration_since(epoch).as_secs_f64();

    // create a pattern of tag/marker
    let mut pat = [0_u8; 16];
    pat[0..8].copy_from_slice(LAT_TAG);
    pat[8..16].copy_from_slice(&now.to_le_bytes());

    // copy the tag/marker pattern repeatedly into the buffer
    let mut offset = 0;
    while offset < buf.len() {
        let len = std::cmp::min(pat.len(), buf.len() - offset);
        buf[offset..offset + len].copy_from_slice(&pat[..len]);
        offset += len;
    }
}

/// Return the duration since the time encoded in a latency info buffer.
/// Returns a unit error if we could not parse the buffer into time data.
#[allow(clippy::result_unit_err)]
pub fn parse_latency_info(buf: &[u8]) -> Result<std::time::Duration, ()> {
    // if the buffer is smaller than 16 bytes, we cannot decode it
    if buf.len() < 16 {
        return Err(());
    }

    // look for a tag, read the next bytes as latency info
    for i in 0..buf.len() - 15 {
        if &buf[i..i + 8] == LAT_TAG {
            let mut time = [0; 8];
            time.copy_from_slice(&buf[i + 8..i + 16]);
            let time = f64::from_le_bytes(time);
            let now = tokio::time::Instant::now();
            let now = now.duration_since(*LOC_EPOCH).as_secs_f64();
            let time = std::time::Duration::from_secs_f64(now - time);
            return Ok(time);
        }
    }
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_latency_buffer_sizes() {
        for i in 0..16 {
            let mut buf = vec![0; i];
            fill_with_latency_info(&mut buf);
            assert!(parse_latency_info(&buf).is_err());
        }
    }

    #[test]
    fn test_bad_latency_buffer_data() {
        assert!(parse_latency_info(&[0; 64]).is_err());
    }

    #[test]
    fn test_good_latency_buffers() {
        for i in 16..64 {
            let mut buf = vec![0; i];
            fill_with_latency_info(&mut buf);
            let val = parse_latency_info(&buf).unwrap();
            assert!(val.as_micros() < 10_000);
        }
    }
}
