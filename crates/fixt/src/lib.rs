pub mod bool;
pub mod number;
pub mod prelude;
pub mod string;
pub mod unit;

#[derive(Clone)]
/// the Fixturator is the struct that we need to impl Iterator for
/// each combination of Item and Curve needs its own Iterator implementation for Fixturator
/// Item represents the type that we want to generate fixtures for
/// Curve represents some algorithm capable of generating fixtures
/// the Item is PhantomData because it simply represents a type to output
/// the Curve must be provided when the Fixturator is constructed to allow for paramaterized curves
/// for example, we could implement a step function over `Foo(u32)` with a `Step(u32)` curve:
///
/// ```rust
/// use fixt::prelude::*;
/// pub struct Step(u32);
/// pub struct Foo(u32);
/// impl Fixt for Foo {};
/// impl Iterator for Fixturator<Foo, Step> {
///     type Item = Foo;
///
///     fn next(&mut self) -> Option<Self::Item> {
///         let ret = Some(Foo(self.index as u32 * self.curve.0));
///         self.index = self.index + 1;
///         ret
///     }
/// }
/// // the first argument to new() is the curve, the second is the starting index
/// let mut fixturator = Fixturator::<u32, Step>::new(Step(5), 0);
/// assert_eq!(fixturator.next().unwrap(), Foo(0));
/// assert_eq!(fixturator.next().unwrap(), Foo(5)); // jumps according to paramaterised curve
///
/// // this syntax is the same thing due to the Fixt impl
/// let mut fixturator = Foo::fixturator(Step(5));
/// assert_eq!(fixturator.next().unwrap(), Foo(0));
/// assert_eq!(fixturator.next().unwrap(), Foo(5));
/// ```
///
/// The inner index is always a single usize.
/// It can be ignored, e.g. in the case of Unpredictable implementations based on `rand::random()`.
/// If it is used it should be incremented by 1 and/or wrapped back to 0 to derive returned values.
/// Ideally the Curve should allow for efficient calculation of a fixture from any given index,
/// e.g. a fibbonacci curve would be a bad idea as it requires sequential/recursive calculations to
/// reach any specific index, c.f. the direct multiplication in the step function above.
/// Following this standard allows for wrapper structs to delegate their curves to the curves of
/// their inner types by constructing an inner Fixturator directly with the outer index passed in.
/// @see newtype_fixt! macro defined below for an example of this.
///
/// Fixturator implements Clone for convenience but note that this will clone the current index.
///
/// Fixturators are lazy and infinite, they must never fail to iterate
/// That is to say, calling fixturator.next().unwrap() must be safe to do and never panic
/// This makes the external interface as easy to compose as possible when building up Fixturators
/// over complex data types that include different curves with various periods.
/// For example, the Predictable bool sequence cycles between true/false with period of 2 while the
/// Predictable string sequence has 10 sample strings that it iterates over. We want to be able to
/// easily support Fixturators over structs containing both string and bool fields, so we wrap the
/// inner Fixturator sequences to keep producing bools and Strings for as needed (rather than
/// forcing the outer struct to stop after 2 bools or manually implement ad-hoc wrapping).
/// Wrapping logic may be subtle, e.g. mapping between a usize index and a u8 Item where the max
/// values do not align, so it is best to centralise the wrapping behaviour inside the Iterator
/// implementations for each <Item, Curve> combination.
/// If you are implementing an iteration over some finite sequence then wrap the iteration back to
/// the start of the sequence once the index exceeds the sequence's bounds or reset the index to 0
/// after seq.len() iterations.
pub struct Fixturator<Item, Curve> {
    item: std::marker::PhantomData<Item>,
    curve: Curve,
    index: usize,
}

impl<Curve, Item> Fixturator<Item, Curve> {
    /// constructs a Fixturator of type <Item, Curve> from a Curve and starting index
    /// raw calls are a little verbose, e.g. `Fixturator::<u32, Predictable>::new(Predictable, 0)`
    /// @see the `Fixt` trait for an ergomonic `fixturator(Curve)` wrapper method
    /// the starting index is exposed to facilitate wrapper structs to delegate their indexes to
    /// internal Fixturators
    /// @see newtype_fixt! macro below for an example of this
    pub fn new(curve: Curve, start: usize) -> Self {
        Fixturator::<Item, Curve> {
            curve,
            index: start,
            item: std::marker::PhantomData,
        }
    }
}

/// represents an unpredictable curve
///
/// unpredictable curves seek to:
/// - disrupt 'just so' implementations of algorithms that lean too heavily on fragile assumptions
/// - have a high probability of generating common edge cases that developers fail to cover
/// a classic example is broken/forgotten NaN handling in code that uses floats for calculations
///
/// in general this is what we want from our tests, to remind us of where we are _wrong_ about our
/// assumptions in our code.
/// it is likely that you want to use the Unpredictable curve as the defacto choice for testing.
///
/// however, note that unpredictable curves are NOT intended:
/// - to comprehensively cover any particular value space
/// - to replace property/fuzz testing
/// - to algorithmically explore edge-cases in an automated fashion
/// - to assert any particular security or correctness concern
///
/// unpredictable curves are a great way to knock off some low hanging fruit, especially around
/// numeric calculations and utf-8 handling, but are no replacement for stringent approaches.
#[derive(Clone)]
pub struct Unpredictable;

/// represents a predictable curve
///
/// a predictable curve simply iterates over some known progression of values in the same way every
/// test run.
///
/// predictable curves can be convenient, or even necessary, if an unpredictable curve breaks our
/// ability to make specific assertions about our code.
///
/// for example, we may want to demonstrate that additon works.
/// with an unpredictable curve we can assert things like the arguments being commutative,
/// associative, additive, etc. but then we quickly end up doing a bad version of property testing.
/// better to assert known expected results of addition from various values from a predictable
/// curve and then subject the addition function to real property testing with a dedicated tool.
///
/// this curve is provided as a standard option because there is a real, common tradeoff between
/// test fragility (accuracy) and specificity (precision).
#[derive(Clone)]
pub struct Predictable;

/// represents a curve over the empty value(s)
/// the concept of "empty" is as slippery as it is of dubious value
/// how many countless hours and bugs have we lost over deciding what "is" and what "isn't"?
/// i'm looking at you, JS and PHP -_-
///
/// regardless, collections with no items, numbers with no magnitude, strings with no chars are all
/// common sources of bugs, so feel free to manifest as much emptiness as you like from this curve.
#[derive(Clone)]
pub struct Empty;

/// an ergonomic wrapper around the raw Fixturator::<Item, Curve>::new(curve, index) verbosity
/// the default implementation of Fixt exposes a single, simple `foo.fixturator(curve)` method
pub trait Fixt {
    fn fixturator<Curve>(curve: Curve) -> Fixturator<Self, Curve>
    where
        Self: Sized,
    {
        Fixturator::<Self, Curve>::new(curve, 0)
    }
}

/// set of basic tests that can be used to test any Fixt implementation
/// usage:
/// - type: the Item of the Fixturator and Fixt impl to be tested
/// - empty_expected: what value should the Empty curve return (assumes a singular Empty)
/// - predictable_expected: vector of any length (can wrap) that we predict from Predictable
/// - test_unpredictable (optional): whether to try and test the unpredictable case
/// @see the tests in modules in this crate
#[macro_export]
macro_rules! basic_test {
    ( $type:ty, $empty_expected:expr, $predictable_expected:expr ) => {
        basic_test!($type, $empty_expected, $predictable_expected, true);
    };
    ( $type:ty, $empty_expected:expr, $predictable_expected:expr, $test_unpredictable:literal ) => {
        paste::item! {
            #[test]
            fn [<$type:lower _empty>] () {
                // we can generate 100 empty values and that they are all empty
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
                // we can predict some vector of values from the Predictable curve
                assert_eq!(
                    $predictable_expected,
                    fixturator.take($predictable_expected.len()).collect::<Vec<$type>>(),
                );
            }
        }

        paste::item! {
            #[test]
            fn [<$type:lower _unpredictable>] () {
                if $test_unpredictable {
                    let empty = <$type>::fixturator(Empty);
                    let unpredictable = <$type>::fixturator(Unpredictable);

                    // the Unpredictable curve is not Empty
                    assert_ne!(
                        empty.take(100).collect::<Vec<$type>>(),
                        unpredictable.take(100).collect::<Vec<$type>>(),
                    );

                    let predictable = <$type>::fixturator(Predictable);
                    let unpredictable = <$type>::fixturator(Unpredictable);

                    // the Unpredictable curve is not Predictable
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
/// IMPORTANT: the inner type needs to implement .into() to the outer type
macro_rules! newtype_fixt {
    // implements a single Fixturator curve
    ( $outer:ident<$inner:ident>, $curve:ident ) => {
        impl Iterator for Fixturator<$outer, $curve> {
            type Item = $outer;

            fn next(&mut self) -> Option<Self::Item> {
                // constructs a Fixturator off the $inner type and inits it at the current index
                // of self so we can delegate the next() call to the $inner type and wrap it in
                // the $outer type via an .into() call
                let ret = Fixturator::<$inner, $curve>::new($curve, self.index)
                    .next()
                    .map(|v| v.into());
                self.index = self.index + 1;
                ret
            }
        }
    };
    // implements all standard Fixturator curves AND Fixt trait for the outer newtype
    ( $outer:ident<$inner:ident> ) => {
        impl Fixt for $outer {}
        newtype_fixt!($outer<$inner>, Empty);
        newtype_fixt!($outer<$inner>, Unpredictable);
        newtype_fixt!($outer<$inner>, Predictable);
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
    newtype_fixt!(MyNewType<bool>);
    basic_test!(
        MyNewType,
        MyNewType(false),
        vec![true, false, true, false, true, false, true, false, true, false]
            .into_iter()
            .map(|b| MyNewType(b))
            .collect::<Vec<MyNewType>>()
    );
}
