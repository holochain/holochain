//! KitsuneP2p Proxy Wire Protocol Items.

use crate::*;

/// Type used to correlate proxy message requests / responses.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Deref,
    AsRef,
    From,
    Into,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct MsgId(pub u64);

static NEXT_MSG_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl MsgId {
    /// Generate the next process-unique channel id.
    pub fn next() -> Self {
        Self(NEXT_MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// Type used to denote a logical proxy channel.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Deref,
    AsRef,
    From,
    Into,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct ChannelId(pub u64);

static NEXT_CHANNEL_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl ChannelId {
    /// Generate the next process-unique channel id.
    pub fn next() -> Self {
        Self(NEXT_CHANNEL_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// Type used for content data of wire proxy messages.
#[derive(Debug, PartialEq, Deref, AsRef, From, Into, serde::Serialize, serde::Deserialize)]
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

#[cfg(test)]
macro_rules! test_val {
    ($($t:ty {$v:expr},)*) => {
        trait TestVal {
            fn test_val() -> Self;
        }

        $(
            impl TestVal for $t {
                fn test_val() -> Self {
                    $v
                }
            }
        )*
    };
}

/// This macro DRYs out implementing the wire protocol variants
/// as there is a lot of shared code between them.
///
/// DSL:
///
/// $s_name - snake-case name
/// $c_name - camel-case name
/// $b      - protocol variant identifier byte (u8) literal
/// $t_name - type name (snake-case)
/// $t_idx  - type index in the message array
/// $t_ty   - type rust type
///
/// Docs allowed on variant and types.
///
/// E.g.:
///
/// write_proxy_wire! {
///    /// Forward data through the proxy channel.
///    /// Send zero length data for keep-alive.
///    chan_send::ChanSend(0x30) {
///        /// The channel id to send data through.
///        (channel_id::0): ChannelId,
///
///        /// The data content to be sent.
///        (channel_data::1): ChannelData,
///    },
/// }
macro_rules! write_proxy_wire {
    ($(
        $(#[doc = $doc:expr])* $s_name:ident :: $c_name:ident($b:literal) {$(
            $(#[doc = $t_doc:expr])* ($t_name:ident :: $t_idx:tt): $t_ty:ty,
        )*},
    )*) => {
        pub(crate) mod type_bytes {$(
            #[allow(non_upper_case_globals)]
            pub(crate) const $c_name: u8 = $b;
        )*}

        /// Proxy Wire Protocol Top-Level Enum.
        #[derive(Debug, PartialEq)]
        #[non_exhaustive]
        pub enum ProxyWire {$(
            $(#[doc = $doc])*
            $c_name($c_name),
        )*}

        impl ProxyWire {
            $(
                /// Create a new instance of this type.
                pub fn $s_name($(
                    $t_name: $t_ty,
                )*) -> Self {
                    Self::$c_name($c_name::new($($t_name,)*))
                }
            )*

            /// Encode this wire message.
            pub fn encode(&self) -> TransportResult<Vec<u8>> {
                use serde::Serialize;
                let mut se = rmp_serde::encode::Serializer::new(Vec::new())
                    .with_struct_map()
                    .with_string_variants();
                let (s, u) = match self {$(
                    Self::$c_name(s) => (s.serialize(&mut se), type_bytes::$c_name),
                )*};
                s.map_err(TransportError::other)?;
                let mut out = se.into_inner();
                out.insert(0, u);
                Ok(out)
            }

            /// Decode a wire message.
            pub fn decode(data: &[u8]) -> TransportResult<Self> {
                Ok(match data[0] {
                    $(
                        type_bytes::$c_name => Self::$c_name(
                            rmp_serde::from_read_ref(&data[1..]).map_err(TransportError::other)?,
                        ),
                    )*
                    _ => return Err("corrupt wire message".into()),
                })
            }
        }

        $(
            $(#[doc = $doc])*
            #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
            pub struct $c_name($(
                $(#[doc = $t_doc])* pub $t_ty,
            )*);

            impl From<$c_name> for ($($t_ty,)*) {
                fn from(o: $c_name) -> Self {
                    o.into_inner()
                }
            }

            impl $c_name {
                /// Create a new instance of this type.
                pub fn new($(
                    $t_name: $t_ty,
                )*) -> Self {
                    Self($($t_name,)*)
                }

                /// Extract the contents of this type.
                pub fn into_inner(self) -> ($($t_ty,)*) {
                    ($(self.$t_idx,)*)
                }
            }
        )*

        #[cfg(test)]
        mod encode_decode_tests {
            use super::*;

            test_val! {
                String { "test".to_string() },
                WireUrl { "test://test".into() },
                MsgId { 42.into() },
                ChannelId { 42.into() },
                ChannelData { vec![0xdb; 32].into() },
            }

            $(
                #[test]
                fn $s_name() {
                    $(
                        let $t_name: $t_ty = TestVal::test_val();
                    )*
                    let msg: ProxyWire = ProxyWire::$s_name($(
                        $t_name,
                    )*);
                    let enc = msg.encode().unwrap();
                    let dec = ProxyWire::decode(&enc).unwrap();
                    assert_eq!(msg, dec);
                }
            )*
        }
    };
}

write_proxy_wire! {
    /// Request that the remote end proxy for us.
    req_proxy::ReqProxy(0x10) {
        /// The message id for this proxy request.
        (msg_id::0): MsgId,
    },

    /// The remote end agrees to proxy for us.
    req_proxy_ok::ReqProxyOk(0x11) {
        /// The message id for this proxy request.
        (msg_id::0): MsgId,

        /// The granted proxy address we can now be reached at.
        (proxy_url::1): WireUrl,
    },

    /// The remote will not proxy for us.
    req_proxy_err::ReqProxyErr(0x12) {
        /// The message id for this proxy request.
        (msg_id::0): MsgId,

        /// The reason why the remote end has rejected our proxy request.
        (reason::1): String,
    },

    /// Create a new proxy channel through which to send data.
    chan_new::ChanNew(0x20) {
        /// The message id for this proxy request.
        (msg_id::0): MsgId,

        /// The destination endpoint for this proxy channel.
        (proxy_url::1): WireUrl,
    },

    /// Create a new proxy channel through which to send data.
    chan_new_ok::ChanNewOk(0x21) {
        /// The message id for this proxy request.
        (msg_id::0): MsgId,

        /// The channel id of the newly created channel.
        (channel_id::1): ChannelId,
    },

    /// Create a new proxy channel through which to send data.
    chan_new_err::ChanNewErr(0x22) {
        /// The message id for this proxy request.
        (msg_id::0): MsgId,

        /// The reason why the channel was not created.
        (reason::1): String,
    },

    /// Forward data through the proxy channel.
    /// Send zero length data for keep-alive.
    chan_send::ChanSend(0x30) {
        /// The channel id to send data through.
        (channel_id::0): ChannelId,

        /// The data content to be sent.
        (channel_data::1): ChannelData,
    },

    /// Close proxy channel.
    /// Channels are bi-directional, this closes one direction, not the other.
    chan_drop::ChanDrop(0x40) {
        /// The channel id to drop.
        (channel_id::0): ChannelId,
    },
}
