pub mod bool;
pub mod number;
pub mod prelude;
pub mod string;
pub mod unit;

#[derive(Clone)]
pub struct Fixturator<Item, Curve> {
    item: std::marker::PhantomData<Item>,
    curve: Curve,
    index: usize,
}

impl<Curve, Item> Fixturator<Item, Curve> {
    pub fn new(curve: Curve, start: usize) -> Self {
        Fixturator::<Item, Curve> {
            curve,
            index: start,
            item: std::marker::PhantomData,
        }
    }
}

#[derive(Clone)]
pub struct Unpredictable;
#[derive(Clone)]
pub struct Predictable;
#[derive(Clone)]
pub struct Empty;

pub trait Fixt {
    fn fixturator<Curve>(curve: Curve) -> Fixturator<Self, Curve>
    where
        Self: Sized,
    {
        Fixturator::<Self, Curve>::new(curve, 0)
    }
}

/// set of basic tests that can be used to test any Fixt implementation
/// @see the tests in modules in this crate
#[macro_export]
macro_rules! basic_test {
    ( $type:ty, $empty_expected:expr, $predictable_len:expr, $predictable_expected:expr ) => {
        basic_test!(
            $type,
            $empty_expected,
            $predictable_len,
            $predictable_expected,
            true
        );
    };
    ( $type:ty, $empty_expected:expr, $predictable_len:expr, $predictable_expected:expr, $test_unpredictable:literal ) => {
        paste::item! {
            #[test]
            fn [<$type:lower _empty>] () {
                // this mutable loop allows us to avoid Clone on $empty_expected which may cause
                // problems for new types if we enforced it
                let mut fixturator = <$type>::fixturator(Empty);
                for _ in 0..100 {
                    assert_eq!(
                        &$empty_expected,
                        &fixturator.next().unwrap(),
                    )
                }
            }
        }

        paste::item! {
            #[test]
            fn [<$type:lower _predictable>] () {
                let fixturator = <$type>::fixturator(Predictable);
                assert_eq!(
                    $predictable_expected,
                    fixturator.take($predictable_len).collect::<Vec<$type>>(),
                );
            }
        }

        paste::item! {
            #[test]
            fn [<$type:lower _unpredictable>] () {
                if $test_unpredictable {
                    let empty = <$type>::fixturator(Empty);
                    let unpredictable = <$type>::fixturator(Unpredictable);

                    assert_ne!(
                        empty.take(100).collect::<Vec<$type>>(),
                        unpredictable.take(100).collect::<Vec<$type>>(),
                    );

                    let predictable = <$type>::fixturator(Predictable);
                    let unpredictable = <$type>::fixturator(Unpredictable);

                    assert_ne!(
                        predictable.take(100).collect::<Vec<$type>>(),
                        unpredictable.take(100).collect::<Vec<$type>>(),
                    );
                }
            }
        }
    };
}

#[macro_export]
/// a direct delegation of fixtures to the inner type for new types
/// IMPORTANT: requires that `From<Inner> for Outer` be implemented for this to work
macro_rules! newtype_fixt {
    // implements a single Fixturator curve
    ( $outer:ty, $inner:ty, $curve:ident ) => {
        impl Iterator for Fixturator<$outer, $curve> {
            type Item = $outer;

            fn next(&mut self) -> Option<Self::Item> {
                let ret = Fixturator::<$inner, $curve>::new($curve, self.index)
                    .next()
                    .map(|i| i.into());
                self.index = self.index + 1;
                ret
            }
        }
    };
    // implements all standard Fixturator curves AND Fixt trait for the outer newtype
    ( $outer:ty, $inner:ty ) => {
        impl Fixt for $outer {}
        newtype_fixt!($outer, $inner, Empty);
        newtype_fixt!($outer, $inner, Unpredictable);
        newtype_fixt!($outer, $inner, Predictable);
    };
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[derive(Debug, PartialEq)]
    struct MyNewType(bool);
    impl From<bool> for MyNewType {
        fn from(b: bool) -> Self {
            Self(b)
        }
    }
    newtype_fixt!(MyNewType, bool);
    basic_test!(
        MyNewType,
        MyNewType(false),
        10,
        vec![true, false, true, false, true, false, true, false, true, false]
            .into_iter()
            .map(|b| MyNewType(b))
            .collect::<Vec<MyNewType>>()
    );
}
