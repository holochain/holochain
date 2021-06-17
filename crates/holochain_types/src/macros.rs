//! General-purpose macros
//! (Consider moving this to its own crate?)

/// Utility for removing boilerplate from From impls
#[macro_export]
macro_rules! impl_from {
    ($($t1:ty => $t2:ty, | $i:ident | {$e:expr},)*) => {$(
        impl From<$t1> for $t2 {
            fn from($i: $t1) -> Self {
                $e
            }
        }
    )*};
}
