#[derive(thiserror::Error, Debug)]
pub enum PewPewPewError {
    #[error(transparent)]
    ActixHttp(#[from] actix_http::error::Error),

    #[error(transparent)]
    HeaderToStr(#[from] actix_web::http::header::ToStrError),

    #[error(transparent)]
    FromHex(#[from] hex::FromHexError),

    #[error(transparent)]
    Var(#[from] std::env::VarError),

    #[error("{0}")]
    RingUnspecified(String),
}

impl actix_http::error::ResponseError for PewPewPewError {}

impl From<ring::error::Unspecified> for PewPewPewError {
    fn from(e: ring::error::Unspecified) -> Self {
        Self::RingUnspecified(format!("{}", e))
    }
}
