//! Encoding / Decoding utilities.

/// Encode a serde::Serialize item as message-pack data to given writer.
/// You may wish to first wrap your writer in a BufWriter.
pub fn rmp_encode<W, S>(write: &mut W, item: S) -> Result<(), std::io::Error>
where
    W: std::io::Write,
    S: serde::Serialize,
{
    let mut se = rmp_serde::encode::Serializer::new(write)
        .with_struct_map()
        .with_string_variants();
    item.serialize(&mut se)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(())
}

/// Decode message-pack data from given reader into an owned item.
/// You may wish to first wrap your reader in a BufReader.
pub fn rmp_decode<R, D>(r: &mut R) -> Result<D, std::io::Error>
where
    R: std::io::Read,
    for<'de> D: Sized + serde::Deserialize<'de>,
{
    let mut de = rmp_serde::decode::Deserializer::new(r);
    D::deserialize(&mut de).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

/// Apply to a data item to indicate it can be encoded / decoded.
pub trait Codec: Clone + Sized {
    /// Variant identifier (for debugging or as a cheap discriminant).
    fn variant_type(&self) -> &'static str;

    /// Encode this item to given writer.
    /// You may wish to first wrap your writer in a BufWriter.
    fn encode<W>(&self, w: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write;

    /// Encode this item to an owned vector of bytes.
    /// Uses `encode()` internally.
    fn encode_vec(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut data = Vec::new();
        self.encode(&mut data)?;
        Ok(data)
    }

    /// Decode a reader into this item.
    /// You may wish to first wrap your reader in a BufReader.
    fn decode<R>(r: &mut R) -> Result<Self, std::io::Error>
    where
        R: std::io::Read;

    /// Decode a range of bytes into this item.
    /// Will also return the byte count read.
    /// Uses `decode()` internally.
    fn decode_ref(r: &[u8]) -> Result<(u64, Self), std::io::Error> {
        let mut r = std::io::Cursor::new(r);
        let item = Self::decode(&mut r)?;
        Ok((r.position(), item))
    }
}

/// Alias for Codec plus the necessary additional trait bounds
pub trait CodecBound: Codec + 'static + Send + std::fmt::Debug {}
impl<T> CodecBound for T where T: Codec + 'static + Send + std::fmt::Debug {}

/// DSL-style macro for generating a serialization protocol message enum.
///
/// DSL:
///
/// ```ignore
/// /// [codec doc here]
/// codec $codec_name {
///     /// [var doc here]
///     $var_name($var_id) {
///         /// [type doc here]
///         $type_name.$type_idx: $type_ty,
///     },
/// }
/// ```
///
/// - $codec_name - camel-case codec enum name
/// - $var_name   - camel-case variant/struct name
/// - $var_id     - protocol variant identifier byte (u8) literal
/// - $type_name  - snake-case type name
/// - $type_idx   - zero-index type index in message array (usize)
/// - $type_ty    - type rust type
///
/// E.G.:
///
/// ```ignore
/// /// My codec is awesome.
/// codec MyCodec {
///     /// My codec has only one variant.
///     MyVariant(0x00) {
///         /// My variant has only one type
///         my_type.0: String,
///     },
/// }
/// ```
#[macro_export]
macro_rules! write_codec_enum {
    ($(#[doc = $codec_doc:expr])* codec $codec_name:ident {$(
        $(#[doc = $var_doc:expr])* $var_name:ident($var_id:literal) {$(
            $(#[doc = $type_doc:expr])* $type_name:ident.$type_idx:literal: $type_ty:ty,
        )*},
    )*}) => {
        $crate::dependencies::paste::item! {
            $(
                $(#[doc = $var_doc])*
                #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
                pub struct [< $var_name:camel >] {
                    $(
                        $(#[doc = $type_doc])* pub [< $type_name:snake >]: $type_ty,
                    )*
                }

                impl $crate::codec::Codec for [< $var_name:camel >] {
                    fn variant_type(&self) -> &'static str {
                        concat!(
                            stringify!([< $codec_name:camel >]),
                            "::",
                            stringify!([< $var_name:camel >]),
                        )
                    }

                    fn encode<W>(&self, w: &mut W) -> ::std::io::Result<()>
                    where
                        W: ::std::io::Write
                    {
                        #[cfg(debug_assertions)]
                        #[allow(dead_code)]
                        {
                            const MSG: &str = "type index must begin at 0 and increment by exactly 1 per type - switching type order will break parsing compatibility";
                            let mut _idx = -1;
                            $(
                                _idx += 1;
                                assert_eq!(_idx, $type_idx, "{}", MSG);
                            )*
                        }
                        let t: (
                            $(&$type_ty,)*
                        ) = (
                            $(&self.[< $type_name:snake >],)*
                        );
                        $crate::codec::rmp_encode(w, &t)
                    }

                    fn decode<R>(r: &mut R) -> ::std::io::Result<Self>
                    where
                        R: ::std::io::Read
                    {
                        let (
                            $([< $type_name:snake >],)*
                        ): (
                            $($type_ty,)*
                        ) = $crate::codec::rmp_decode(r)?;
                        Ok([< $var_name:camel >] {
                            $(
                                [< $type_name:snake >],
                            )*
                        })
                    }
                }
            )*

            $(#[doc = $codec_doc])*
            #[derive(Clone, Debug, PartialEq)]
            pub enum [< $codec_name:camel >] {
                $(
                    $(#[doc = $var_doc])*
                    [< $var_name:camel >]([< $var_name:camel >]),
                )*
            }

            impl [< $codec_name:camel >] {
                $(
                    /// Variant constructor helper function.
                    pub fn [< $var_name:snake >]($(
                        [< $type_name:snake >]: $type_ty,
                    )*) -> Self {
                        Self::[< $var_name:camel >]([< $var_name:camel >] {
                            $(
                                [< $type_name:snake >],
                            )*
                        })
                    }
                )*
            }

            impl $crate::codec::Codec for [< $codec_name:camel >] {
                fn variant_type(&self) -> &'static str {
                    match self {
                        $(
                            Self::[< $var_name:camel >](data) =>
                                $crate::codec::Codec::variant_type(data),
                        )*
                    }
                }

                fn encode<W>(&self, w: &mut W) -> ::std::io::Result<()>
                where
                    W: ::std::io::Write
                {
                    match self {
                        $(
                            Self::[< $var_name:camel >](data) => {
                                ::std::io::Write::write_all(w, &[$var_id])?;
                                $crate::codec::Codec::encode(data, w)
                            }
                        )*
                    }
                }

                fn decode<R>(r: &mut R) -> Result<Self, ::std::io::Error>
                where
                    R: ::std::io::Read
                {
                    let mut c = [0_u8; 1];
                    ::std::io::Read::read_exact(r, &mut c)?;
                    match c[0] {
                        $(
                            $var_id => {
                                Ok(Self::[< $var_name:camel >]($crate::codec::Codec::decode(r)?))
                            },
                        )*
                        _ => Err(::std::io::Error::new(::std::io::ErrorKind::Other, "invalid protocol byte")),
                    }
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use super::*;
    use std::sync::Arc;

    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct Sub(pub Vec<u8>);

    write_codec_enum! {
        /// Codec
        codec Bob {
            /// variant
            BobOne(0x42) {
                /// type 1
                yay.0: bool,

                /// type 2
                age.1: u32,

                /// type 3
                sub.2: Arc<Sub>,
            },
            /// nother variant
            BobTwo(0x43) {
            },
        }
    }

    #[test]
    fn test_encode_decode() {
        let bob = Bob::bob_one(true, 42, Arc::new(Sub(b"test".to_vec())));
        let data = bob.encode_vec().unwrap();
        let res = Bob::decode_ref(&data).unwrap().1;
        assert_eq!(bob, res);
    }
}
