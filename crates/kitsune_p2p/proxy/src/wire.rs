//! KitsuneP2p Proxy Wire Protocol Items.

use crate::*;

/// Type used for content data of wire proxy messages.
#[derive(
    Debug, Clone, PartialEq, Deref, AsRef, From, Into, serde::Serialize, serde::Deserialize,
)]
pub struct ChannelData(#[serde(with = "serde_bytes")] pub Vec<u8>);

/// Wire type for transfering urls.
#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, serde::Serialize, serde::Deserialize)]
pub struct WireUrl(String);

impl WireUrl {
    /// Convert to url2.
    pub fn to_url(&self) -> ProxyUrl {
        self.into()
    }

    /// Convert to url2.
    pub fn into_url(self) -> ProxyUrl {
        self.into()
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
       String => WireUrl,      |s| { Self(s) },
      &String => WireUrl,      |s| { Self(s.to_string()) },
         &str => WireUrl,      |s| { Self(s.to_string()) },
     ProxyUrl => WireUrl,    |url| { Self(url.to_string()) },
    &ProxyUrl => WireUrl,    |url| { Self(url.to_string()) },
      WireUrl => ProxyUrl,   |url| { url.0.into() },
     &WireUrl => ProxyUrl,   |url| { (&url.0).into() },
}

kitsune_p2p_types::write_codec_enum! {
    /// Proxy Wire Protocol Top-Level Enum.
    codec ProxyWire {
        /// Indicate a failur on the remote end.
        Failure(0x02) {
            /// Text description reason describing remote failure.
            reason.0: String,
        },

        /// Request that the remote end proxy for us.
        ReqProxy(0x10) {
            /// The cert digest others should expect when tunnelling TLS
            cert_digest.0: ChannelData,
        },

        /// The remote end agrees to proxy for us.
        ReqProxyOk(0x11) {
            /// The granted proxy address we can now be reached at.
            proxy_url.0: WireUrl,
        },

        /// Create a new proxy channel through which to send data.
        ChanNew(0x20) {
            /// The destination endpoint for this proxy channel.
            proxy_url.0: WireUrl,
        },

        /// Forward data through the proxy channel.
        /// Send zero length data for keep-alive.
        ChanSend(0x30) {
            /// The data content to be sent.
            channel_data.0: ChannelData,
        },
    }
}
