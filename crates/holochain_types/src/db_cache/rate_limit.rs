//! Cached DB state for rate limiting

pub struct RateLimitDbCache {

}


impl RateLimitDbCache {
    pub fn process_header(&mut self, header: &Header) {
        let author = header.author();
        let bucket_id = header.rate_
    }
}