const GITHUB_SIGNATURE_HEADER_NAME: &str = "X-Hub-Signature";
const GITHUB_SIGNATURE_HEADER_PREFIX: &str = "sha1=";
const GITHUB_WEBHOOK_SECRET_ENV_NAME: &str = "GITHUB_WEBHOOK_SECRET";

pub fn verify(
    request: &actix_web::HttpRequest,
    body: &str,
) -> Result<(), crate::error::PewPewPewError> {
    let header = request
        .headers()
        .get(GITHUB_SIGNATURE_HEADER_NAME)
        .ok_or(actix_web::error::ErrorUnauthorized(
            actix_web::error::ParseError::Header,
        ))?
        .to_str()?;
    let signature: Vec<u8> = hex::decode(&header[GITHUB_SIGNATURE_HEADER_PREFIX.len()..])?;

    let secret = std::env::var(GITHUB_WEBHOOK_SECRET_ENV_NAME)?;

    let s_key = ring::hmac::Key::new(ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret.as_bytes());

    Ok(ring::hmac::verify(
        &s_key,
        body.as_bytes(),
        signature.as_ref(),
    )?)
}
