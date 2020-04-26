use rand::seq::SliceRandom;
use rand::Rng;

macro_rules! fixt_unsigned {
    ( $t:ty ) => {
        impl $crate::Fixt for $t {}

        impl Iterator for $crate::Fixturator<$t, $crate::Unpredictable> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                let polite: bool = rand::random();
                if polite {
                    Some(rand::random())
                } else {
                    let impolite_vals = vec![<$t>::max_value(), <$t>::min_value(), 1];
                    impolite_vals.choose(&mut rand::thread_rng()).cloned()
                }
            }
        }

        impl Iterator for $crate::Fixturator<$t, $crate::Predictable> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                // this casting and wrapping gets around the fact that usize for the index won't
                // line up with uX probably
                // uBIG as uSMALL will cast to USMALL::MAX so e.g. `usize as u8` will cap at 255
                // wrapping_add(1) brings us back to 0 when we hit the cap of $t
                // this doesn't allow us to iterate past usize::MAX if uX is bigger than usize
                let ret = self.index as $t;
                self.index = ret.wrapping_add(1) as usize;
                Some(ret)
            }
        }

        impl Iterator for $crate::Fixturator<$t, $crate::Empty> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                Some(0)
            }
        }

        paste::item! {
            pub struct [<UnpredictableRange $t:upper>]($t, $t);
            pub struct [<PredictableRange $t:upper>]($t, $t);

            impl Iterator for $crate::Fixturator<$t, [<UnpredictableRange $t:upper>]> {
                type Item = $t;

                fn next(&mut self) -> Option<Self::Item> {
                    let mut rng = rand::thread_rng();
                    Some(rng.gen_range(self.curve.0, self.curve.1))
                }
            }
            impl Iterator for $crate::Fixturator<$t, [<PredictableRange $t:upper>]> {
                type Item = $t;

                fn next(&mut self) -> Option<Self::Item> {
                    self.index = self.index + 1;
                    Some(((self.index - 1) as $t % self.curve.1) + self.curve.0)
                }
            }
        }
    };
}

fixt_unsigned!(u8);
fixt_unsigned!(u16);
fixt_unsigned!(u32);
fixt_unsigned!(u64);
fixt_unsigned!(u128);
fixt_unsigned!(usize);

macro_rules! fixt_signed {
    ( $t:ty ) => {
        impl $crate::Fixt for $t {}

        impl Iterator for $crate::Fixturator<$t, $crate::Unpredictable> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                let polite: bool = rand::random();
                if polite {
                    Some(rand::random())
                } else {
                    let impolite_vals = vec![<$t>::max_value(), <$t>::min_value(), 1, 0];
                    impolite_vals.choose(&mut rand::thread_rng()).cloned()
                }
            }
        }

        impl Iterator for $crate::Fixturator<$t, $crate::Predictable> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                // this casting and wrapping gets around the fact that usize for the index won't
                // line up with uX probably
                // uBIG as uSMALL will cast to USMALL::MAX so e.g. `usize as u8` will cap at 255
                // wrapping_add(1) brings us back to 0 when we hit the cap of $t
                // this doesn't allow us to iterate past usize::MAX if uX is bigger than usize
                let ret = self.index as $t;
                self.index = ret.wrapping_add(1) as usize;

                // let odds be negative and evens positive
                let signed_ret = if ret % 2 == 0 { ret } else { -ret };
                Some(signed_ret)
            }
        }

        impl Iterator for $crate::Fixturator<$t, $crate::Empty> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                Some(0)
            }
        }

        paste::item! {
            pub struct [<UnpredictableRange $t:upper>]($t, $t);
            pub struct [<PredictableRange $t:upper>]($t, $t);

            impl Iterator for $crate::Fixturator<$t, [<UnpredictableRange $t:upper>]> {
                type Item = $t;

                fn next(&mut self) -> Option<Self::Item> {
                    let mut rng = rand::thread_rng();
                    Some(rng.gen_range(self.curve.0, self.curve.1))
                }
            }
            impl Iterator for $crate::Fixturator<$t, [<PredictableRange $t:upper>]> {
                type Item = $t;

                fn next(&mut self) -> Option<Self::Item> {
                    self.index = self.index + 1;
                    Some(((self.index - 1) as $t % self.curve.1) + self.curve.0)
                }
            }
        }
    };
}

fixt_signed!(i8);
fixt_signed!(i16);
fixt_signed!(i32);
fixt_signed!(i64);
fixt_signed!(i128);
fixt_signed!(isize);

macro_rules! fixt_float {
    ( $t:ident ) => {
        impl $crate::Fixt for $t {}

        impl Iterator for $crate::Fixturator<$t, $crate::Unpredictable> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                let polite: bool = rand::random();
                if polite {
                    Some(rand::random())
                } else {
                    let impolite_vals = vec![
                        std::$t::NEG_INFINITY,
                        std::$t::INFINITY,
                        std::$t::NAN,
                        -1.0,
                        1.0,
                        0.0,
                    ];
                    impolite_vals.choose(&mut rand::thread_rng()).cloned()
                }
            }
        }

        impl Iterator for $crate::Fixturator<$t, $crate::Predictable> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                let ret = self.index as $t;

                // let odds be negative point five and evens positive
                let signed_ret = if self.index % 2 == 0 { ret } else { -ret - 0.5 };
                self.index = self.index + 1;
                Some(signed_ret)
            }
        }

        impl Iterator for $crate::Fixturator<$t, $crate::Empty> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                Some(0.0)
            }
        }

        paste::item! {
            pub struct [<UnpredictableRange $t:upper>]($t, $t);
            pub struct [<PredictableRange $t:upper>]($t, $t);

            impl Iterator for $crate::Fixturator<$t, [<UnpredictableRange $t:upper>]> {
                type Item = $t;

                fn next(&mut self) -> Option<Self::Item> {
                    let mut rng = rand::thread_rng();
                    Some(rng.gen_range(self.curve.0, self.curve.1))
                }
            }
            impl Iterator for $crate::Fixturator<$t, [<PredictableRange $t:upper>]> {
                type Item = $t;

                fn next(&mut self) -> Option<Self::Item> {
                    self.index = self.index + 1;
                    Some(((self.index - 1) as $t % self.curve.1) + self.curve.0)
                }
            }
        }
    };
}

fixt_float!(f32);
fixt_float!(f64);

#[cfg(test)]
pub mod tests {
    use crate::prelude::*;

    basic_test!(u8, 0, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    basic_test!(u16, 0, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    basic_test!(u32, 0, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    basic_test!(u64, 0, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    basic_test!(u128, 0, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    basic_test!(usize, 0, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

    basic_test!(i8, 0, vec![0, -1, 2, -3, 4, -5, 6, -7, 8, -9]);
    basic_test!(i16, 0, vec![0, -1, 2, -3, 4, -5, 6, -7, 8, -9]);
    basic_test!(i32, 0, vec![0, -1, 2, -3, 4, -5, 6, -7, 8, -9]);
    basic_test!(i64, 0, vec![0, -1, 2, -3, 4, -5, 6, -7, 8, -9]);
    basic_test!(i128, 0, vec![0, -1, 2, -3, 4, -5, 6, -7, 8, -9]);
    basic_test!(isize, 0, vec![0, -1, 2, -3, 4, -5, 6, -7, 8, -9]);

    basic_test!(
        f32,
        0.0,
        vec![0.0, -1.5, 2.0, -3.5, 4.0, -5.5, 6.0, -7.5, 8.0, -9.5]
    );
    basic_test!(
        f64,
        0.0,
        vec![0.0, -1.5, 2.0, -3.5, 4.0, -5.5, 6.0, -7.5, 8.0, -9.5]
    );
}
