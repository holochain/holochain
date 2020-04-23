use rand::Rng;

pub enum FixTT<I: Sized> {
    /// some empty value, like ""
    Empty,
    /// some fixed value
    /// probably a "foo"
    /// used as the Default
    A,
    /// another fixed value, different from A
    B,
    /// another fixed value, different to both A and B
    C,
    /// random data
    /// this is NOT intended to replace fuzz/property testing
    /// the goal is to make test data unpredictable to avoid "just so" implementations
    /// there is no intent to comprehensively cover fixture-space or seek out edge cases
    /// there is no intent to hit any particular statistical distribution or range of values
    /// typically the expectation is that it falls back to whatever `rand::random()` does
    Random,
    /// opens fixture implementations up for extension
    /// a fixture sub-type
    Input(I),
}

impl<I: Sized> Default for FixTT<I> {
    fn default() -> Self {
        Self::Empty
    }
}

pub trait FixT {
    /// @TODO it would be nice to provide a default Input type if/when that becomes available
    /// @see https://github.com/rust-lang/rust/issues/29661
    /// type Input: Sized = ();
    type Input: Sized;
    fn fixt(fixtt: FixTT<Self::Input>) -> Self;
    fn fixt_empty() -> Self
    where
        Self: Sized,
    {
        Self::fixt(FixTT::Empty)
    }
    fn fixt_a() -> Self
    where
        Self: Sized,
    {
        Self::fixt(FixTT::A)
    }
    fn fixt_b() -> Self
    where
        Self: Sized,
    {
        Self::fixt(FixTT::B)
    }
    fn fixt_c() -> Self
    where
        Self: Sized,
    {
        Self::fixt(FixTT::C)
    }
    fn fixt_random() -> Self
    where
        Self: Sized,
    {
        Self::fixt(FixTT::Random)
    }
    fn fixt_input(input: Self::Input) -> Self
    where
        Self: Sized,
    {
        Self::fixt(FixTT::Input(input))
    }
}

impl FixT for () {
    type Input = ();
    fn fixt(_: FixTT<Self::Input>) -> Self {
        ()
    }
}

impl FixT for bool {
    type Input = ();
    fn fixt(fixtt: FixTT<Self::Input>) -> Self {
        match fixtt {
            FixTT::A => false,
            FixTT::B => true,
            // there is no third option for bool!
            FixTT::C => unimplemented!(),
            FixTT::Empty => false,
            FixTT::Random => rand::random(),
            FixTT::Input(_) => unimplemented!(),
        }
    }
}

macro_rules! fixt_uint {
    ( $t:ty, $tt:ident ) => {
        pub enum $tt {
            Range($t, $t),
        }
        impl FixT for $t {
            type Input = $tt;
            fn fixt(fixtt: FixTT<Self::Input>) -> Self {
                match fixtt {
                    FixTT::Empty => 0,
                    FixTT::A => <$t>::min_value(),
                    FixTT::B => 1,
                    FixTT::C => <$t>::max_value(),
                    FixTT::Random => rand::random(),
                    FixTT::Input(fixt_uint) => match fixt_uint {
                        $tt::Range(min, max) => {
                            let mut rng = rand::thread_rng();
                            rng.gen_range(min, max)
                        }
                    },
                }
            }
        }
    };
}

fixt_uint!(u8, FixTU8);
fixt_uint!(u16, FixTU16);
fixt_uint!(u32, FixTU32);
fixt_uint!(u64, FixTU64);
fixt_uint!(u128, FixTU128);
fixt_uint!(usize, FixTUSize);

macro_rules! fixt_iint {
    ( $t:ty, $tt:ident ) => {
        pub enum $tt {
            Range($t, $t),
        }
        impl FixT for $t {
            type Input = $tt;
            fn fixt(fixtt: FixTT<Self::Input>) -> Self {
                match fixtt {
                    FixTT::Empty => 0,
                    FixTT::A => <$t>::min_value(),
                    FixTT::B => 0,
                    FixTT::C => <$t>::max_value(),
                    FixTT::Random => rand::random(),
                    FixTT::Input(fixt_uint) => match fixt_uint {
                        $tt::Range(min, max) => {
                            let mut rng = rand::thread_rng();
                            rng.gen_range(min, max)
                        }
                    },
                }
            }
        }
    };
}

fixt_iint!(i8, FixTI8);
fixt_iint!(i16, FixTI16);
fixt_iint!(i32, FixTI32);
fixt_iint!(i64, FixTI64);
fixt_iint!(i128, FixTI128);
fixt_iint!(isize, FixTISize);

#[cfg(test)]
mod tests {
    use crate::FixTU128;
    use crate::FixTU16;
    use crate::FixTU32;
    use crate::FixTU64;
    use crate::FixTU8;
    use crate::FixTUSize;
    use crate::FixTI128;
    use crate::FixTI16;
    use crate::FixTI32;
    use crate::FixTI64;
    use crate::FixTI8;
    use crate::FixTISize;
    use crate::{FixT, FixTT};
    use hamcrest2::prelude::*;
    use rstest::rstest;

    macro_rules! basic_test {
        ( $f:ident, $t:ty, $tt:ty, $d:expr, $e:expr, $a:expr, $b:expr, $c:expr ) => {
            #[rstest(
                i,
                o,
                case(FixTT::default(), $e),
                case(FixTT::Empty, $e),
                case(FixTT::A, $a),
                case(FixTT::B, $b),
                case(FixTT::C, $c)
            )]
            fn $f(i: FixTT<$tt>, o: $t) {
                match i {
                    FixTT::Empty => assert_that!(&<$t>::fixt_empty(), eq(&o)),
                    FixTT::A => assert_that!(&<$t>::fixt_a(), eq(&o)),
                    FixTT::B => assert_that!(&<$t>::fixt_b(), eq(&o)),
                    FixTT::C => assert_that!(&<$t>::fixt_c(), eq(&o)),
                    _ => {}
                }
                assert_that!(&<$t>::fixt(i), eq(&o));
            }
        };
    }

    // function name, type to test, input type, empty, a, b, c
    basic_test!(unit_test, (), (), (), (), (), (), ());

    macro_rules! uint_test {
        ( $f:ident, $t:ty, $tt:ty ) => {
            basic_test!( $f, $t, $tt, 0, 0, <$t>::min_value(), 1, <$t>::max_value() );
        };
    }

    uint_test!(u8_test, u8, FixTU8);
    uint_test!(u16_test, u16, FixTU16);
    uint_test!(u32_test, u32, FixTU32);
    uint_test!(u64_test, u64, FixTU64);
    uint_test!(u128_test, u128, FixTU128);
    uint_test!(usize_test, usize, FixTUSize);

    macro_rules! iint_test {
        ( $f:ident, $t:ty, $tt:ty ) => {
            basic_test!( $f, $t, $tt, 0, 0, <$t>::min_value(), 0, <$t>::max_value() );
        };
    }

    iint_test!(i8_test, i8, FixTI8);
    iint_test!(i16_test, i16, FixTI16);
    iint_test!(i32_test, i32, FixTI32);
    iint_test!(i64_test, i64, FixTI64);
    iint_test!(i128_test, i128, FixTI128);
    iint_test!(isize_test, isize, FixTISize);

    basic_test!(
        new_type_test,
        MyNewType,
        FixTU32,
        MyNewType(0),
        MyNewType(0),
        MyNewType(<u32>::min_value()),
        MyNewType(1),
        MyNewType(<u32>::max_value())
    );

    /// show an example of a newtype delegating to inner fixtures
    #[derive(Debug, PartialEq)]
    struct MyNewType(u32);
    impl FixT for MyNewType {
        type Input = FixTU32;
        fn fixt(fixtt: FixTT<Self::Input>) -> Self {
            Self(u32::fixt(fixtt))
        }
    }
}
