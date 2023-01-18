#![allow(clippy::assign_op_pattern)]

pub mod bool;
pub mod bytes;
pub mod number;
pub mod prelude;
#[deny(missing_docs)]
mod rng;
pub mod serialized_bytes;
pub mod string;
pub mod unit;
pub use paste;

pub use rng::rng;

/// the Fixturator is the struct that we wrap in our FooFixturator newtypes to impl Iterator over
/// each combination of Item and Curve needs its own Iterator implementation for Fixturator
/// Item is the Foo type of FooFixturator, i.e. the type of thing we are generating examples of
/// Curve represents some algorithm capable of generating fixtures
/// the Item is PhantomData because it simply represents a type to output
/// the Curve must be provided when the Fixturator is constructed to allow for paramaterized curves
/// this is most easily handled in most cases with the fixturator! and newtype_fixturator! macros
///
/// The inner index is always a single usize.
/// It can be ignored, e.g. in the case of Unpredictable implementations based on `rand::random()`.
/// If it is used it should be incremented by 1 and/or wrapped back to 0 to derive returned values.
/// Ideally the Curve should allow for efficient calculation of a fixture from any given index,
/// e.g. a fibbonacci curve would be a bad idea as it requires sequential/recursive calculations to
/// reach any specific index, c.f. the direct multiplication in the step function above.
/// Following this standard allows for wrapper structs to delegate their curves to the curves of
/// their inner types by constructing an inner Fixturator directly with the outer index passed in.
/// If we can always assume the inner fixturators can be efficiently constructed at any index this
/// allows us to efficiently compose fixturators.
/// See [ `newtype_fixturator!` ](newtype_fixturator) macro defined below for an example of this.
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
/// essentially, the iteration of a fixturator should work like some_iter.cycle()
pub struct Fixturator<Item, Curve> {
    item: std::marker::PhantomData<Item>,
    pub curve: Curve,
    pub index: usize,
}

impl<Curve, Item> Fixturator<Item, Curve> {
    /// constructs a Fixturator of type <Item, Curve> from a Curve and starting index
    /// raw calls are a little verbose, e.g. `Fixturator::<u32, Predictable>::new(Predictable, 0)`
    /// the starting index is exposed to facilitate wrapper structs to delegate their indexes to
    /// internal Fixturators
    /// See [`newtype_fixturator!`](newtype_fixturator) macro below for an example of this
    pub fn new(curve: Curve, start: usize) -> Self {
        Fixturator::<Item, Curve> {
            curve,
            index: start,
            item: std::marker::PhantomData,
        }
    }
}

// /// set of basic tests that can be used to test any FooFixturator implementation
// /// usage:
// /// - type: the Foo of FooFixturator to be tested
// /// - empty_expected: vector of any length of empties that we predict from Empty
// /// - predictable_expected: vector of any length (can wrap) that we predict from Predictable
// /// - test_unpredictable (optional): whether to try and test the unpredictable case
// /// See the tests in modules in this crate
#[macro_export]
macro_rules! basic_test {
    ( $type:ty, $empty_expected:expr, $predictable_expected:expr ) => {
        basic_test!($type, $empty_expected, $predictable_expected, true);
    };
    ( $type:ty, $empty_expected:expr, $predictable_expected:expr, $test_unpredictable:literal ) => {
        $crate::prelude::paste! {
            #[test]
            #[cfg(test)]
            fn [<$type:lower _empty>] () {
                let empties = [<$type:camel Fixturator>]::new(Empty);
                // we can make many empties from the Empty curve
                assert_eq!(
                    $empty_expected,
                    empties.take($empty_expected.len()).collect::<Vec<$type>>(),
                );
            }
        }

        $crate::prelude::paste! {
            #[test]
            #[cfg(test)]
            fn [<$type:lower _predictable>] () {
                let predictables = [<$type:camel Fixturator>]::new($crate::prelude::Predictable);
                // we can predict some vector of values from the Predictable curve
                assert_eq!(
                    $predictable_expected,
                    predictables.take($predictable_expected.len()).collect::<Vec<$type>>(),
                );
            }
        }

        $crate::prelude::paste! {
            #[test]
            #[cfg(test)]
            fn [<$type:lower _unpredictable>] () {
                if $test_unpredictable {
                    let empties = [<$type:camel Fixturator>]::new(Empty);
                    let unpredictables = [<$type:camel Fixturator>]::new($crate::prelude::Unpredictable);

                    // the Unpredictable curve is not Empty
                    assert_ne!(
                        empties.take(100).collect::<Vec<$type>>(),
                        unpredictables.take(100).collect::<Vec<$type>>(),
                    );

                    let predictables = [<$type:camel Fixturator>]::new($crate::prelude::Predictable);
                    let unpredictables = [<$type:camel Fixturator>]::new($crate::prelude::Unpredictable);

                    // the Unpredictable curve is not Predictable
                    assert_ne!(
                        predictables.take(100).collect::<Vec<$type>>(),
                        unpredictables.take(100).collect::<Vec<$type>>(),
                    );
                }
            }
        }
    };
}

/// implements a FooFixturator for any type Foo
/// this simply wraps `Fixturator<Foo, Curve>` up as `FooFixturator<Curve>`
///
/// this macro serves a few purposes:
/// - we avoid the orphan rule that would prevent us implementing Iterator on Fixturator directly
/// - we avoid the verbosity of type and impl juggling around every new FooFixturator
/// - we create a FooFixturator implementation that is compatible with basic_test! macro
/// - we cover all three basic curves
/// - we standardiize the new() and new_indexed() methods without relying on traits
///
/// the expressions passed into the macro are the body of the next calls for Empty, Unpredictable
/// and Predictable, in order
#[macro_export]
macro_rules! fixturator {
    (
        with_vec $min:literal $max:literal;
        $type:tt;
        $($munch:tt)*
    ) => {
        $crate::prelude::paste! {
            pub type [<$type:camel Vec>] = Vec<$type>;
            fixturator!(
                [<$type:camel Vec>];
                curve Empty vec![];
                curve Unpredictable {
                    let mut index = get_fixt_index!();
                    let mut rng = $crate::rng();
                    let len = rng.gen_range($min..$max);
                    let mut fixturator = [<$type:camel Fixturator>]::new_indexed($crate::prelude::Unpredictable, index);
                    let mut v = vec![];
                    for _ in 0..len {
                        v.push(fixturator.next().unwrap());
                    }
                    index += 1;
                    set_fixt_index!(index);
                    v
                };
                curve Predictable {
                    let mut index = get_fixt_index!();
                    let mut fixturator = [<$type:camel Fixturator>]::new_indexed($crate::prelude::Predictable, index);
                    let mut v = vec![];
                    let min = $min;
                    let max = (index % ($max - min)) + min;
                    for _ in min..max {
                        v.push(fixturator.next().unwrap());
                    }
                    index += 1;
                    set_fixt_index!(index);
                    v
                };
            );
        }
        fixturator!($type; $($munch)*);
    };

    // for an enum Foo with variants with a single inner type
    //
    // fixturator!(Foo; variants [ A(String) B(bool) ];);
    //
    // implements all basic curves using fixturators for the variant inner types
    (
        $type:tt;
        variants [ $( $variant:tt($variant_inner:ty) )* ];
        $($munch:tt)*
    ) => {

        fixturator!(
            $type;
            enum [ $( $variant )* ];

            curve Empty $crate::prelude::paste! { match [<$type:camel Variant>]::random() {
                $(
                    [<$type:camel Variant>]::$variant => $type::$variant(
                        [<$variant_inner:camel Fixturator>]::new_indexed($crate::prelude::Empty, get_fixt_index!()).next().unwrap().into()
                    ),
                )*
            }};

            curve Unpredictable $crate::prelude::paste! { match [<$type:camel Variant>]::random() {
                $(
                    [<$type:camel Variant>]::$variant => $type::$variant(
                        [<$variant_inner:camel Fixturator>]::new_indexed($crate::prelude::Unpredictable, get_fixt_index!()).next().unwrap().into()
                    ),
                )*
            }};

            curve Predictable $crate::prelude::paste! { match [<$type:camel Variant>]::nth(get_fixt_index!()) {
                $(
                    [<$type:camel Variant>]::$variant => $type::$variant(
                        [<$variant_inner:camel Fixturator>]::new_indexed($crate::prelude::Predictable, get_fixt_index!()).next().unwrap().into()
                    ),
                )*
            }};

            $($munch)*
        );
    };

    // for an enum Foo with unit variants with no inner types
    //
    // fixturator!(Foo; unit variants [ A B ] empty B;);
    //
    // implements all basic curves returning the empty curve passed to the macro, or a random
    // variant or an iterating variant from the index
    (
        $type:tt;
        unit variants [ $( $variant:tt )* ] empty $empty:tt;
        $($munch:tt)*
    ) => {
        fixturator!(
            $type;
            enum [ $( $variant )* ];
            curve Empty {
                $crate::prelude::paste! { $type::$empty }
            };
            curve Unpredictable $crate::prelude::paste! { match [<$type:camel Variant>]::random() {
                $(
                        [<$type:camel Variant>]::$variant => $type::$variant,
                )*
            }};
            curve Predictable $crate::prelude::paste! {{
                match [<$type:camel Variant>]::nth(get_fixt_index!()) {
                $(
                    [<$type:camel Variant>]::$variant => $type::$variant,
                )*
            }}};
            $($munch)*
        );
    };

    // for any complex enum
    //
    // fixturator!(Foo; enum [ A B ]; curve ...; curve ...; curve ...;);
    //
    // implements an enum with variants matching Foo as FooVariant
    // this enum can be iterated over as per the strum crate EnumIter
    //
    // it also has convenience methods to match against:
    //
    // - FooVariant::random() for a random variant of Foo
    // - FooVariant::nth(n) for an indexed variant of Foo
    //
    // See the tests in this file for examples.
        (
            $type:tt;
            enum [ $( $variant:tt )* ];
            $($munch:tt)*
        ) => {
            $crate::prelude::paste! {
                #[derive($crate::prelude::strum_macros::EnumIter)]
                enum [<$type:camel Variant>] {
                    $( $variant ),*
                }

                impl [<$type:camel Variant>] {
                    fn random() -> Self {
                        [<$type:camel Variant>]::iter().choose(&mut $crate::rng()).unwrap()
                    }
                    fn nth(index: usize) -> Self {
                        $crate::prelude::paste! {
                            [<$type:camel Variant>]::iter().cycle().nth(index).unwrap()
                        }
                    }
                }
            }

            fixturator!($type; $($munch)* );
    };

    // for any Foo that impl From<Bar>
    //
    // fixturator!(Foo; from Bar;);
    //
    // implements all the curves by building Foo from a BarFixturator
    ( $type:ident; from $from:ty; $($munch:tt)* ) => {
        fixturator!(
            $type;

            curve Empty {
                $type::from(
                    $crate::prelude::paste! {
                        [< $from:camel Fixturator >]::new_indexed($crate::prelude::Empty, get_fixt_index!()).next().unwrap()
                    }
                )
            };
            curve Unpredictable {
                $type::from(
                    $crate::prelude::paste! {
                        [< $from:camel Fixturator >]::new_indexed($crate::prelude::Unpredictable, get_fixt_index!()).next().unwrap()
                    }
                )
            };
            curve Predictable {
                $type::from(
                    $crate::prelude::paste! {
                        [< $from:camel Fixturator >]::new_indexed($crate::prelude::Predictable, get_fixt_index!()).next().unwrap()
                    }
                )
            };
        );
    };

    // for any Foo that has a constructor function like Foo::new( ... )
    //
    // fixturator!(Foo; constructor fn new(String, String, bool););
    //
    // implements all curves by building all the arguments to the named constructor function from
    // the fixturators of the types specified to the macro
    ( $type:ident; constructor fn $fn:tt( $( $newtype:ty ),* ); $($munch:tt)* ) => {
        fixturator!(
            $type;

            curve Empty {
                let index = get_fixt_index!();
                $type::$fn(
                    $(
                        $crate::prelude::paste! {
                            [< $newtype:camel Fixturator >]::new_indexed($crate::prelude::Empty, index).next().unwrap().into()
                        }
                    ),*
                )
            };

            curve Unpredictable {
                let index = get_fixt_index!();
                $type::$fn(
                    $(
                        $crate::prelude::paste! {
                            [< $newtype:camel Fixturator >]::new_indexed($crate::prelude::Unpredictable, index).next().unwrap().into()
                        }
                    ),*
                )
            };
            curve Predictable {
                let index = get_fixt_index!();
                $type::$fn(
                    $(
                        $crate::prelude::paste! {
                            [< $newtype:camel Fixturator >]::new_indexed($crate::prelude::Predictable, index).next().unwrap().into()
                        }
                    ),*
                )
            };

            $($munch)*
        );
    };

    // for any Foo that has a vanilla function like fn make_foo( ... ) -> Foo
    //
    // fixturator!(Foo; vanilla fn make_foo(String, String, bool););
    //
    // implements all curves by building all the arguments to the named vanilla function from
    // the fixturators of the types specified to the macro
    ( $type:ident; vanilla fn $fn:tt( $( $newtype:ty ),* ); $($munch:tt)* ) => {
        fixturator!(
            $type;

            curve Empty {
                $fn(
                    $(
                        $crate::prelude::paste! {
                            [< $newtype:camel Fixturator >]::new_indexed($crate::prelude::Empty, get_fixt_index!()).next().unwrap().into()
                        }
                    ),*
                )
            };

            curve Unpredictable {
                $fn(
                    $(
                        $crate::prelude::paste! {
                            [< $newtype:camel Fixturator >]::new_indexed($crate::prelude::Unpredictable, get_fixt_index!()).next().unwrap().into()
                        }
                    ),*
                )
            };
            curve Predictable {
                $fn(
                    $(
                        $crate::prelude::paste! {
                            [< $newtype:camel Fixturator >]::new_indexed($crate::prelude::Predictable, get_fixt_index!()).next().unwrap().into()
                        }
                    ),*
                )
            };

            $($munch)*
        );
    };

    // implement a single curve for Foo
    //
    // fixturator!(Foo; curve MyCurve { ... };);
    //
    // uses TT munching for multiple curves
    // used internally by this macro for all baseline curves
    // See https://danielkeep.github.io/tlborm/book/pat-incremental-tt-munchers.html
    ( $type:ident; curve $curve:ident $e:expr; $($munch:tt)* ) => {
        curve!( $type, $curve, $e);

        fixturator!( $type; $($munch)* );
    };

    // create a FooFixturator for Foo
    //
    // fixturator!(Foo;);
    //
    // simply creates a newtype around the standard Fixturator struct and implements two methods:
    // - FooFixturator::new(curve) to construct a FooFixturator with curve at index 0
    // - FooFixturator::new(curve, index) to construct a FooFixturator with curve at index
    //
    // intended to be the TT munch endpoint for all patterns in this macro
    // See https://danielkeep.github.io/tlborm/book/pat-incremental-tt-munchers.html
    ( $type:ident; $($munch:tt)* ) => {
        $crate::prelude::paste! {
            #[allow(missing_docs)]
            pub struct [<$type:camel Fixturator>]<Curve>(Fixturator<$type, Curve>);

            #[allow(missing_docs)]
            impl <Curve>[<$type:camel Fixturator>]<Curve> {
                pub fn new(curve: Curve) -> [<$type:camel Fixturator>]<Curve> {
                    Self::new_indexed(curve, 0)
                }
                pub fn new_indexed(curve: Curve, start: usize) -> [<$type:camel Fixturator>]<Curve> {
                    [<$type:camel Fixturator>](Fixturator::<$type, Curve>::new(curve, start))
                }
            }
        }
    };

    // legacy syntax
    //
    // fixturator!(Foo, { /* empty */ }, { /* unpredictable */ }, { /* predictable */ });
    //
    // implements both FooFixturator and all the curves from raw expressions passed to the macro
    //
    // this syntax has several limitations:
    // - positional curve definitions are easy to accidentally mix up
    // - couples all curve definitions together and to FooFixturator creation
    // - undifferentiated logic forces much boilerplate because the macro knows nothing about Foo
    // - forces devs to define curves that might not be needed or make sense yet
    ( $type:ident, $empty:expr, $unpredictable:expr, $predictable:expr ) => {
        fixturator!(
            $type;
            curve Empty $empty;
            curve Unpredictable $unpredictable;
            curve Predictable $predictable;
        );
    };
}

#[macro_export]
macro_rules! get_fixt_index {
    () => {{
        let mut index = 0;
        FIXT_INDEX.with(|f| index = *f.borrow());
        index
    }};
}

#[macro_export]
macro_rules! set_fixt_index {
    ($index:expr) => {{
        FIXT_INDEX.with(|f| *f.borrow_mut() = $index);
    }};
}

#[macro_export]
macro_rules! get_fixt_curve {
    () => {{
        let mut curve = None;
        FIXT_CURVE.with(|f| curve = f.borrow().clone());
        curve.unwrap()
    }};
}

#[macro_export]
/// implement Iterator for a FooFixturator for a given curve
///
/// curve!(Foo, Unpredictable, /* make an Unpredictable Foo here */ );
///
/// simple wrapper around the standard Iterator trait from rust
/// the expression in the third parameter to curve! is just the body of .next() without the need or
/// ability to return an Option - i.e. return a value of type Foo _not_ `Option<Foo>`
/// if the body of the expression changes the index it will be respected, if not then it will be
/// incremented by 1 automatically by the macro
macro_rules! curve {
    ( $type:ident, $curve:ident, $e:expr ) => {
        $crate::prelude::paste! {
            #[allow(missing_docs)]
            impl Iterator for [< $type:camel Fixturator >]<$curve> {
                type Item = $type;

                fn next(&mut self) -> Option<Self::Item> {
                    thread_local!(static FIXT_INDEX: std::cell::RefCell<usize> = std::cell::RefCell::new(0));
                    thread_local!(static FIXT_CURVE: std::cell::RefCell<Option<$curve>> = std::cell::RefCell::new(None));
                    FIXT_INDEX.with(|f| *f.borrow_mut() = self.0.index);
                    FIXT_CURVE.with(|f| *f.borrow_mut() = Some(self.0.curve.clone()));
                    let original_index = self.0.index;
                    let ret = $e;
                    FIXT_INDEX.with(|f| self.0.index = *f.borrow());
                    if original_index == self.0.index {
                        self.0.index += 1;
                    }
                    Some(ret)
                }
            }
        }
    };
}

#[macro_export]
/// tiny convenience macro to make it easy to get the first Foo from its fixturator without using
/// the iterator interface to save a little typing
/// c.f. fixt!(Foo) vs. FooFixturator::new(Unpredictable).next().unwrap();
macro_rules! fixt {
    ( $name:tt ) => {
        $crate::fixt!($name, $crate::prelude::Unpredictable)
    };
    ( $name:tt, $curve:expr ) => {
        $crate::fixt!($name, $curve, 0)
    };
    ( $name:tt, $curve:expr, $index:expr ) => {
        $crate::prelude::paste! { [< $name:camel Fixturator>]::new_indexed($curve, $index).next().unwrap() }
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
#[derive(Clone, Copy)]
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
#[derive(Clone, Copy)]
pub struct Predictable;

/// represents a curve over the empty value(s)
/// the concept of "empty" is as slippery as it is of dubious value
/// how many countless hours and bugs have we lost over deciding what "is" and what "isn't"?
/// i'm looking at you, JS and PHP -_-
///
/// regardless, collections with no items, numbers with no magnitude, strings with no chars are all
/// common sources of bugs, so feel free to manifest as much emptiness as you like from this curve.
#[derive(Clone, Copy)]
pub struct Empty;

#[macro_export]
/// a direct delegation of fixtures to the inner type for new types
macro_rules! newtype_fixturator {
    ( $outer:ident<Vec<$inner:ty>> ) => {
        fixturator!(
            $outer,
            $outer(vec![]),
            {
                let mut rng = $crate::rng();
                let vec_len = rng.gen_range(0..5);
                let mut ret = vec![];
                let mut inner_fixturator =
                    $crate::prelude::paste! { [<$inner:camel Fixturator>]::new_indexed($crate::prelude::Unpredictable, get_fixt_index!()) };
                for _ in 0..vec_len {
                    ret.push(inner_fixturator.next().unwrap());
                }
                set_fixt_index!(get_fixt_index!() + 1);
                $outer(ret)
            },
            {
                let mut rng = $crate::rng();
                let vec_len = rng.gen_range(0..5);
                let mut ret = vec![];
                let mut inner_fixturator =
                    $crate::prelude::paste! { [<$inner:camel Fixturator>]::new_indexed($crate::prelude::Predictable, get_fixt_index!()) };
                for _ in 0..vec_len {
                    ret.push(inner_fixturator.next().unwrap());
                }
                set_fixt_index!(get_fixt_index!() + 1);
                $outer(ret)
            }
        );
    };
    ( $outer:ident<$inner:ty> ) => {
        fixturator!(
            $outer,
            {
                let mut index = get_fixt_index!();
                let mut fixturator =
                    $crate::prelude::paste! { [<$inner:camel Fixturator>]::new_indexed($crate::prelude::Empty, index) };
                index += 1;
                set_fixt_index!(index);
                $outer(fixturator.next().unwrap())
            },
            {
                let mut index = get_fixt_index!();
                let mut fixturator =
                    $crate::prelude::paste! { [<$inner:camel Fixturator>]::new_indexed($crate::prelude::Unpredictable, index) };
                index += 1;
                set_fixt_index!(index);
                $outer(fixturator.next().unwrap())
            },
            {
                let mut index = get_fixt_index!();
                let mut fixturator =
                    $crate::prelude::paste! { [<$inner:camel Fixturator>]::new_indexed($crate::prelude::Predictable, index) };
                index += 1;
                set_fixt_index!(index);
                $outer(fixturator.next().unwrap())
            }
        );
    };
}

#[macro_export]
/// a direct delegation of fixtures to the inner type for wasm io types
/// See zome types crate
macro_rules! wasm_io_fixturator {
    ( $outer:ident<$inner:ty> ) => {
        fixturator!(
            $outer,
            {
                let mut fixturator =
                    $crate::prelude::paste! { [<$inner:camel Fixturator>]::new_indexed($crate::prelude::Empty, get_fixt_index!()) };
                set_fixt_index!(get_fixt_index!() + 1);
                $outer::new(fixturator.next().unwrap())
            },
            {
                let mut fixturator =
                    $crate::prelude::paste! { [<$inner:camel Fixturator>]::new_indexed($crate::prelude::Unpredictable, get_fixt_index!()) };
                set_fixt_index!(get_fixt_index!() + 1);
                $outer::new(fixturator.next().unwrap())
            },
            {
                let mut fixturator =
                    $crate::prelude::paste! { [<$inner:camel Fixturator>]::new_indexed($crate::prelude::Predictable, get_fixt_index!()) };
                set_fixt_index!(get_fixt_index!() + 1);
                $outer::new(fixturator.next().unwrap())
            }
        );
    };
}

#[macro_export]
/// Creates a simple way to generate enums that use the strum way of iterating
/// <https://docs.rs/strum/0.18.0/strum/>
/// iterates over all the variants (Predictable) or selects random variants (Unpredictable)
/// You do still need to BYO "empty" variant as the macro doesn't know what to use there
macro_rules! enum_fixturator {
    ( $enum:ident, $empty:expr ) => {
        use rand::seq::IteratorRandom;
        use $crate::prelude::IntoEnumIterator;
        fixturator!(
            $enum,
            $empty,
            { $enum::iter().choose(&mut $crate::rng()).unwrap() },
            {
                let ret = $enum::iter().cycle().nth(self.0.index).unwrap();
                set_fixt_index!(get_fixt_index!() + 1);
                ret
            }
        );
    };
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use crate::string::PREDICTABLE_STRS;

    // in general enums can have a mix of whatever in their variants
    #[derive(PartialEq, Debug)]
    pub enum Foo {
        A,
        B(String),
    }

    fixturator!(
        Foo;
        enum [ A B ];
        curve Empty Foo::A;
        curve Unpredictable match FooVariant::random() {
            FooVariant::A => Foo::A,
            FooVariant::B => Foo::B(fixt!(String)),
        };
        curve Predictable match FooVariant::nth(get_fixt_index!()) {
            FooVariant::A => Foo::A,
            FooVariant::B => Foo::B(StringFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap()),
        };
    );

    #[test]
    fn enum_test() {
        assert_eq!(FooFixturator::new(Predictable).next().unwrap(), Foo::A,);

        FooFixturator::new(Unpredictable).next().unwrap();

        assert_eq!(FooFixturator::new(Empty).next().unwrap(), Foo::A,);

        let mut fixt_iter = FooFixturator::new(Predictable);
        assert_eq!(fixt_iter.next().unwrap(), Foo::A);
        let string = StringFixturator::new_indexed(Predictable, 1)
            .next()
            .unwrap();
        assert_eq!(fixt_iter.next().unwrap(), Foo::B(string));
    }

    #[derive(PartialEq, Debug)]
    pub enum UnitFoo {
        A,
        B,
        C,
    }

    fixturator!(
        UnitFoo;
        unit variants [ A B C ] empty B;
    );

    #[test]
    fn unit_variants_test() {
        assert_eq!(
            UnitFooFixturator::new(Predictable).next().unwrap(),
            UnitFoo::A,
        );

        // smoke test Unpredictable
        UnitFooFixturator::new(Unpredictable).next().unwrap();

        assert_eq!(UnitFooFixturator::new(Empty).next().unwrap(), UnitFoo::B,);
    }

    #[derive(PartialEq, Debug, Clone)]
    pub enum VariantFoo {
        A(String),
        B(usize),
        C(bool),
    }

    fixturator!(
        VariantFoo;
        variants [ A(String) B(usize) C(bool) ];
    );

    #[test]
    fn variant_variants_test() {
        let mut predictable_fixturator = VariantFooFixturator::new(Predictable);
        for expected in [
            VariantFoo::A("💯".into()),
            VariantFoo::B(1),
            VariantFoo::C(true),
            VariantFoo::A(".".into()),
            VariantFoo::B(4),
            VariantFoo::C(false),
        ]
        .iter()
        {
            assert_eq!(expected.to_owned(), predictable_fixturator.next().unwrap(),);
        }

        let mut unpredictable_fixturator = VariantFooFixturator::new(Unpredictable);
        for _ in 0..10 {
            // smoke test
            unpredictable_fixturator.next().unwrap();
        }

        let mut empty_fixturator = VariantFooFixturator::new(Empty);
        for _ in 0..10 {
            match empty_fixturator.next().unwrap() {
                VariantFoo::A(s) => assert_eq!(s, ""),
                VariantFoo::B(n) => assert_eq!(n, 0),
                VariantFoo::C(b) => assert!(!b),
            }
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct StringFoo(String);

    impl From<String> for StringFoo {
        fn from(s: String) -> Self {
            Self(s)
        }
    }

    fixturator!(StringFoo; from String;);

    #[test]
    fn from_test() {
        let mut predictable_fixturator = StringFooFixturator::new(Predictable);
        for expected in PREDICTABLE_STRS.iter() {
            assert_eq!(
                StringFoo::from(expected.to_string()),
                predictable_fixturator.next().unwrap()
            );
        }

        let mut unpredictable_fixturator = StringFooFixturator::new(Unpredictable);
        for _ in 0..10 {
            // smoke test
            unpredictable_fixturator.next().unwrap();
        }

        let mut empty_fixturator = StringFooFixturator::new(Empty);
        for _ in 0..10 {
            assert_eq!(
                StringFoo::from("".to_string()),
                empty_fixturator.next().unwrap(),
            );
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct ConstructedFoo {
        bar: bool,
    }

    impl ConstructedFoo {
        fn from_bar(bar: bool) -> Self {
            Self { bar }
        }
    }

    fixturator!(
        ConstructedFoo;
        constructor fn from_bar(bool);
    );

    #[test]
    fn constructor_test() {
        let mut predictable_fixturator = ConstructedFooFixturator::new(Predictable);
        for expected in [true, false].iter().cycle().take(5) {
            assert_eq!(
                ConstructedFoo::from_bar(*expected),
                predictable_fixturator.next().unwrap(),
            );
        }

        let mut unpredictable_fixturator = ConstructedFooFixturator::new(Unpredictable);
        for _ in 0..10 {
            // smoke test
            unpredictable_fixturator.next().unwrap();
        }

        let mut empty_fixturator = ConstructedFooFixturator::new(Empty);
        for _ in 0..10 {
            assert_eq!(
                ConstructedFoo::from_bar(false),
                empty_fixturator.next().unwrap(),
            );
        }
    }
}
