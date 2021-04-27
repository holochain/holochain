use std::sync::Arc;

/// New-type for sync ref-counted Urls
/// to make passing around tx2 more efficient.
#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct TxUrl(Arc<url2::Url2>);

impl TxUrl {
    /// reference this txurl as a Url2.
    pub fn as_url2(&self) -> &url2::Url2 {
        &self.0
    }
}

impl std::ops::Deref for TxUrl {
    type Target = url::Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for TxUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TxUrl> for Arc<url2::Url2> {
    fn from(u: TxUrl) -> Self {
        u.0
    }
}

impl From<TxUrl> for url2::Url2 {
    fn from(u: TxUrl) -> Self {
        (*u.0).clone()
    }
}

impl From<Arc<url2::Url2>> for TxUrl {
    fn from(r: Arc<url2::Url2>) -> Self {
        Self(r)
    }
}

impl From<url2::Url2> for TxUrl {
    fn from(r: url2::Url2) -> Self {
        Self(Arc::new(r))
    }
}

impl From<String> for TxUrl {
    fn from(r: String) -> Self {
        Self(Arc::new(url2::Url2::parse(&r)))
    }
}

impl From<&String> for TxUrl {
    fn from(r: &String) -> Self {
        Self(Arc::new(url2::Url2::parse(r)))
    }
}

impl From<&str> for TxUrl {
    fn from(r: &str) -> Self {
        Self(Arc::new(url2::Url2::parse(r)))
    }
}
