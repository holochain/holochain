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
    /// random data of size u32
    /// the "size" means whatever it means to the implementation
    /// hopefully something sensible
    /// this is NOT intended to replace fuzz/property testing
    /// the goal is to make test data unpredictable to avoid "just so" implementations
    /// there is no intent to comprehensively cover fixture-space or seek out edge cases
    Random(u32),
    /// opens fixture implementations up for extension
    /// a fixture sub-type
    Input(I),
}

impl<I: Sized> Default for FixTT<I> {
    fn default() -> Self {
        Self::A
    }
}

pub trait FixT {
    type Input: Sized;
    fn fixt(fixtt: FixTT<Self::Input>) -> Self;
}

impl FixT for () {
    type Input = ();
    fn fixt(_: FixTT<Self::Input>) -> Self {
        ()
    }
}

pub enum FixTU32 {
    Range(u32, u32),
}

impl FixT for u32 {
    type Input = FixTU32;
    fn fixt(fixtt: FixTT<Self::Input>) -> Self {
        match fixtt {
            FixTT::Empty => 0,
            FixTT::A => 0,
            FixTT::B => 1,
            FixTT::C => 2,
            FixTT::Random(random) => u32::fixt(FixTT::Input(FixTU32::Range(0, random))),
            FixTT::Input(fixt_u32) => match fixt_u32 {
                FixTU32::Range(min, max) => {
                    let mut rng = rand::thread_rng();
                    rng.gen_range(min, max)
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::FixTU32;
    use crate::{FixT, FixTT};
    use hamcrest2::prelude::*;
    use rstest::rstest;

    #[rstest(
        tt,
        case(FixTT::default()),
        case(FixTT::A),
        case(FixTT::B),
        case(FixTT::C)
    )]
    fn unit_test(tt: FixTT<()>) {
        assert_that!(<()>::fixt(tt), eq(()));
    }

    #[rstest(
        i,
        o,
        case(FixTT::default(), 0),
        case(FixTT::A, 0),
        case(FixTT::B, 1),
        case(FixTT::C, 2),
        case(FixTT::Empty, 0)
    )]
    fn u32_test(i: FixTT<FixTU32>, o: u32) {
        assert_that!(u32::fixt(i), eq(o));
    }

    /// show an example of a newtype delegating to inner fixtures
    #[derive(Debug, PartialEq)]
    struct MyNewType(u32);
    impl FixT for MyNewType {
        type Input = FixTU32;
        fn fixt(fixtt: FixTT<Self::Input>) -> Self {
            Self(u32::fixt(fixtt))
        }
    }
    #[rstest(
        i,
        o,
        case(FixTT::default(), MyNewType(0)),
        case(FixTT::A, MyNewType(0)),
        case(FixTT::B, MyNewType(1)),
        case(FixTT::C, MyNewType(2)),
        case(FixTT::Empty, MyNewType(0))
    )]
    fn new_type_test(i: FixTT<FixTU32>, o: MyNewType) {
        assert_that!(MyNewType::fixt(i), eq(o));
    }
}
