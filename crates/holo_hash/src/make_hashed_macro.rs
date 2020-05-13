#[macro_export]
/// Generate a "Hashed" wrapper struct around a `TryInto<SerializedBytes>` item.
/// Only includes a `with_pre_hashed` constructor.
///
/// `make_hashed_base! { (pub) MyTypeHashed, MyType, holo_hash::EntryHash }`
macro_rules! make_hashed_base {
    (($($vis:tt)*) $n:ident, $t:ty, $h:ty) => {
        /// "Hashed" wrapper type - provides access to the original item,
        /// plus the HoloHash of that item.
        #[derive(::std::fmt::Debug, ::std::clone::Clone)]
        $($vis)* struct $n($crate::GenericHashed<$t, $h>);

        impl $n {
            /// Produce a "Hashed" wrapper with a provided hash.
            pub fn with_pre_hashed(t: $t, h: $h) -> Self {
                Self($crate::GenericHashed::with_pre_hashed(t, h))
            }
        }

        impl $crate::Hashed for $n {
            type Content = $t;
            type HashType = $h;

            fn into_inner(self) -> (Self::Content, Self::HashType) {
                self.0.into_inner()
            }

            fn as_content(&self) -> &Self::Content {
                self.0.as_content()
            }

            fn as_hash(&self) -> &Self::HashType {
                self.0.as_hash()
            }
        }

        impl ::std::convert::From<$n> for ($t, $h) {
            fn from(n: $n) -> ($t, $h) {
                use $crate::Hashed;
                n.into_inner()
            }
        }

        impl ::std::ops::Deref for $n {
            type Target = $t;

            fn deref(&self) -> &Self::Target {
                use $crate::Hashed;
                self.as_content()
            }
        }

        impl ::std::convert::AsRef<$t> for $n {
            fn as_ref(&self) -> &$t {
                use $crate::Hashed;
                self.as_content()
            }
        }

        impl ::std::borrow::Borrow<$t> for $n {
            fn borrow(&self) -> &$t {
                use $crate::Hashed;
                self.as_content()
            }
        }

        impl ::std::convert::AsRef<$h> for $n {
            fn as_ref(&self) -> &$h {
                use $crate::Hashed;
                self.as_hash()
            }
        }

        impl ::std::borrow::Borrow<$h> for $n {
            fn borrow(&self) -> &$h {
                use $crate::Hashed;
                self.as_hash()
            }
        }

        impl ::std::cmp::PartialEq for $n {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl ::std::cmp::Eq for $n {}

        impl ::std::hash::Hash for $n {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
    };
}

#[macro_export]
/// Generate a "Hashed" wrapper struct around a `TryInto<SerializedBytes>` item.
/// Including a `with_data` hashing constructor.
///
/// The purpose of these hashed wrappers is to make an ergonomic and generalized way to create data and cache
/// the calculated hash of that data along with it in a ways that's safe and let's us not have to recalculate it many times.
///
/// The first parameter to the macro is the name of the hashed type usually just the name of type which is passed 
/// as the second parameter with the word `Hashed` added.  The third parameter is kind of hash this type is
/// hashed to which must be a `holo_hash` type.
///
/// `make_hashed! { (pub) MyTypeHashed, MyType, holo_hash::EntryHash }`
macro_rules! make_hashed {
    (($($vis:tt)*) $n:ident, $t:ty, $h:ty) => {
        $crate::make_hashed_base!( ($($vis)*) $n, $t, $h );

        impl $n {
            /// Serialize and hash the given item, producing a "Hashed" wrapper.
            pub async fn with_data(t: $t) -> Result<Self, ::holochain_serialized_bytes::SerializedBytesError> {
                let sb = ::holochain_serialized_bytes::SerializedBytes::try_from(&t)?;
                Ok(Self::with_pre_hashed(t, <$h>::with_data(sb.bytes()).await))
            }
        }
    };
}
