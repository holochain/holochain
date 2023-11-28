use std::sync::Arc;

use crate::{KitsuneError, KitsuneErrorKind, KitsuneResult};

/// New-type for sync ref-counted Urls
/// to make passing around tx2 more efficient.
#[derive(Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TxUrl(Arc<url2::Url2>);

impl TxUrl {
    /// reference this txurl as a Url2.
    pub fn as_url2(&self) -> &url2::Url2 {
        &self.0
    }

    /// Construct from a string which is known to be a valid URL.
    /// Panics if the URL is not parseable.
    pub fn from_str_panicking(s: &str) -> Self {
        url2::Url2::parse(s).into()
    }
}

impl std::ops::Deref for TxUrl {
    type Target = url::Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Debug for TxUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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

impl TryFrom<String> for TxUrl {
    type Error = KitsuneError;

    fn try_from(r: String) -> KitsuneResult<Self> {
        Ok(Self(Arc::new(url2::Url2::try_parse(r.clone()).map_err(
            |e| KitsuneError::from(KitsuneErrorKind::BadInput(Box::new(e), r)),
        )?)))
    }
}

impl TryFrom<&String> for TxUrl {
    type Error = KitsuneError;

    fn try_from(r: &String) -> KitsuneResult<Self> {
        Ok(Self(Arc::new(url2::Url2::try_parse(r).map_err(|e| {
            KitsuneError::from(KitsuneErrorKind::BadInput(Box::new(e), r.clone()))
        })?)))
    }
}

impl TryFrom<&str> for TxUrl {
    type Error = KitsuneError;

    fn try_from(r: &str) -> KitsuneResult<Self> {
        Ok(Self(Arc::new(url2::Url2::try_parse(r).map_err(|e| {
            KitsuneError::from(KitsuneErrorKind::BadInput(Box::new(e), r.to_string()))
        })?)))
    }
}

#[cfg(feature = "fuzzing")]
impl<'a> arbitrary::Arbitrary<'a> for TxUrl {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // XXX: not the best implementation, it doesn't exercise other protocols
        let s = String::arbitrary(u)?;
        Ok(TxUrl::from_str_panicking(&format!("http://{s}")))
    }
}

#[cfg(feature = "fuzzing")]
impl proptest::arbitrary::Arbitrary for TxUrl {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<TxUrl>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::strategy::Strategy;

        // XXX: not the best implementation, it doesn't exercise other protocols
        proptest::string::string_regex(r"http://\w+")
            .unwrap()
            .prop_map(|s| TxUrl::from_str_panicking(&s))
            .boxed()
    }
}
