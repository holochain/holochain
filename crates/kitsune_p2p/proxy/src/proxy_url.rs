//! Utilities for dealing with proxy urls.

use crate::*;

/// Utility for deailing with proxy urls.
#[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deref, AsRef)]
#[display(fmt = "{}", full)]
pub struct ProxyUrl {
    #[deref]
    #[as_ref]
    full: url2::Url2,
    base: url2::Url2,
}

impl ProxyUrl {
    /// Create a new proxy url from a full url str.
    pub fn from_full(full: &str) -> TransportResult<Self> {
        macro_rules! err {
            () => {
                TransportError::from(format!("Invalid Proxy Url: {}", full))
            };
        }
        let full = url2::try_url2!("{}", full).map_err(|_| err!())?;
        let base_scheme = match full.path_segments() {
            None => return Err(err!()),
            Some(mut s) => match s.next() {
                None => return Err(err!()),
                Some(s) => s,
            },
        };
        let mut base = url2::url2!("{}://", base_scheme);
        {
            let mut path = full.path_segments().ok_or_else(|| err!())?;
            path.next();
            let mut mode = 0;
            loop {
                let key = match path.next() {
                    None => break,
                    Some(key) => key,
                };
                if key == "--" {
                    mode = 1;
                    continue;
                }
                match mode {
                    0 => {
                        let val = match path.next() {
                            None => break,
                            Some(val) => val,
                        };
                        match key {
                            "h" => base.set_host(Some(val)).map_err(|_| err!())?,
                            "p" => base
                                .set_port(Some(val.parse().map_err(|_| err!())?))
                                .map_err(|_| err!())?,
                            "u" => base.set_username(val).map_err(|_| err!())?,
                            "w" => base.set_password(Some(val)).map_err(|_| err!())?,
                            _ => return Err(format!("Invalid Proxy Url: {}", full).into()),
                        }
                    }
                    1 => {
                        base.path_segments_mut().map_err(|_| err!())?.push(key);
                    }
                    _ => unreachable!(),
                }
            }
        }
        base.set_query(full.query());
        base.set_fragment(full.fragment());
        Ok(Self { full, base })
    }

    /// Create a new proxy url from a base + tls cert digest.
    pub fn new(base: &str, cert_digest: CertDigest) -> TransportResult<Self> {
        let base = url2::try_url2!("{}", base).map_err(TransportError::other)?;
        let tls = base64::encode_config(&cert_digest[..], base64::URL_SAFE_NO_PAD);
        let mut full = url2::url2!("kitsune-proxy://{}", tls);
        {
            let mut path = full
                .path_segments_mut()
                .map_err(|_| TransportError::from(""))?;
            path.push(base.scheme());
            if let Some(h) = base.host_str() {
                path.push("h");
                path.push(h);
            }
            if let Some(p) = base.port() {
                path.push("p");
                path.push(&format!("{}", p));
            }
            if !base.username().is_empty() {
                path.push("u");
                path.push(base.username());
            }
            if let Some(w) = base.password() {
                path.push("w");
                path.push(w);
            }
            path.push("--");
            if let Some(s) = base.path_segments() {
                for s in s {
                    path.push(s);
                }
            }
        }
        full.set_query(base.query());
        full.set_fragment(base.fragment());
        Ok(Self { full, base })
    }

    /// Get the base url this proxy is addressable at.
    pub fn as_base(&self) -> &url2::Url2 {
        &self.base
    }

    /// Get the base url this proxy is addressable at as a &str reference.
    pub fn as_base_str(&self) -> &str {
        self.base.as_str()
    }

    /// Convert this proxy url instance into a base url.
    pub fn into_base(self) -> url2::Url2 {
        self.base
    }

    /// Get the full url referencing this proxy.
    pub fn as_full(&self) -> &url2::Url2 {
        &self.full
    }

    /// Get the full url referencing this proxy as a &str reference.
    pub fn as_full_str(&self) -> &str {
        self.full.as_str()
    }

    /// Convert this proxy url instance into a full url.
    pub fn into_full(self) -> url2::Url2 {
        self.full
    }

    /// Convert this proxy url instance into a (BaseUrl, FullUrl) tuple.
    pub fn into_inner(self) -> (url2::Url2, url2::Url2) {
        (self.base, self.full)
    }
}

macro_rules! q_from {
    ($($t1:ty => $t2:ty, | $i:ident | {$e:expr},)*) => {$(
        impl From<$t1> for $t2 {
            fn from($i: $t1) -> Self {
                $e
            }
        }
    )*};
}

q_from! {
       ProxyUrl => (url2::Url2, url2::Url2), |url| { url.into_inner() },
      &ProxyUrl => (url2::Url2, url2::Url2), |url| { url.clone().into_inner() },
       ProxyUrl => url2::Url2, |url| { url.into_full() },
      &ProxyUrl => url2::Url2, |url| { url.as_full().clone() },
         String => ProxyUrl, |url| { ProxyUrl::from_full(&url).unwrap() },
        &String => ProxyUrl, |url| { ProxyUrl::from_full(url).unwrap() },
           &str => ProxyUrl, |url| { ProxyUrl::from_full(url).unwrap() },
     url2::Url2 => ProxyUrl, |url| { ProxyUrl::from_full(url.as_str()).unwrap() },
    &url2::Url2 => ProxyUrl, |url| { ProxyUrl::from_full(url.as_str()).unwrap() },
}

impl AsRef<str> for ProxyUrl {
    fn as_ref(&self) -> &str {
        self.as_full_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CERT: &str = "VlyCSmL5WRKUTOLmF9wF0oFy5Jqbxy0I9KPeXqB_9Z4";
    const TEST_FULL: &str = "kitsune-proxy://VlyCSmL5WRKUTOLmF9wF0oFy5Jqbxy0I9KPeXqB_9Z4/kitsune-quic/h/1.2.3.4/p/443/u/bob/w/pass/--/yada1/yada2?c=bla&t=EugO96mIgrCph7QMpqJkkI5BPY5GuIP7JcCshnwh8FY&j=bla#bla";
    const TEST_BASE: &str = "kitsune-quic://bob:pass@1.2.3.4:443/yada1/yada2?c=bla&t=EugO96mIgrCph7QMpqJkkI5BPY5GuIP7JcCshnwh8FY&j=bla#bla";

    #[test]
    fn proxy_url_from_full() {
        let u = ProxyUrl::from_full(TEST_FULL).unwrap();
        assert_eq!(TEST_FULL, u.as_full_str());
        assert_eq!(TEST_BASE, u.as_base_str());
    }

    #[test]
    fn proxy_url_from_base() {
        let cert_digest = base64::decode_config(TEST_CERT, base64::URL_SAFE_NO_PAD).unwrap();
        let u = ProxyUrl::new(TEST_BASE, cert_digest.into()).unwrap();
        assert_eq!(TEST_FULL, u.as_full_str());
        assert_eq!(TEST_BASE, u.as_base_str());
    }
}
