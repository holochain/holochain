use crate::prelude::*;
use rand::seq::SliceRandom;

macro_rules! fixturator_unsigned {
    ( $t:ident ) => {
        fixturator!(
            $t,
            0,
            {
                let mut rng = crate::rng();
                if rng.gen() {
                    rng.gen()
                } else {
                    vec![<$t>::max_value(), <$t>::min_value(), 1]
                        .choose(&mut rng)
                        .unwrap()
                        .to_owned()
                }
            },
            {
                let ret = get_fixt_index!() as $t;
                set_fixt_index!(ret.wrapping_add(1) as usize);
                ret
            }
        );
    };
}

fixturator_unsigned!(u8);
fixturator_unsigned!(u16);
fixturator_unsigned!(u32);
fixturator_unsigned!(u64);
fixturator_unsigned!(u128);
fixturator_unsigned!(usize);

// we can exhaustively enumerate u8 wrapping, which should give us confidence in the u16 behaviour
// given that it uses the same macro
basic_test!(
    u8,
    vec![0; 40],
    (0_u8..=255_u8)
        .into_iter()
        .cycle()
        .take(1000)
        .collect::<Vec<u8>>()
);
basic_test!(
    u16,
    vec![0; 40],
    (0..1000).into_iter().collect::<Vec<u16>>()
);
basic_test!(
    u32,
    vec![0; 40],
    (0..1000).into_iter().collect::<Vec<u32>>()
);
basic_test!(
    u64,
    vec![0; 40],
    (0..1000).into_iter().collect::<Vec<u64>>()
);
basic_test!(
    u128,
    vec![0; 40],
    (0..1000).into_iter().collect::<Vec<u128>>()
);
basic_test!(
    usize,
    vec![0; 40],
    (0..1000).into_iter().collect::<Vec<usize>>()
);

macro_rules! fixturator_signed {
    ( $t:ident ) => {
        fixturator!(
            $t,
            0,
            {
                let mut rng = crate::rng();
                if rng.gen() {
                    rng.gen()
                } else {
                    vec![<$t>::max_value(), <$t>::min_value(), 1]
                        .choose(&mut rng)
                        .unwrap()
                        .to_owned()
                }
            },
            {
                let ret = get_fixt_index!() as $t;
                set_fixt_index!(ret.wrapping_add(1) as usize);
                // negate odds
                let ret = if ret % 2 == 0 { ret } else { -ret };
                ret
            }
        );
    };
}

fixturator_signed!(i8);
fixturator_signed!(i16);
fixturator_signed!(i32);
fixturator_signed!(i64);
fixturator_signed!(i128);
fixturator_signed!(isize);

basic_test!(
    i8,
    vec![0; 40],
    (0_i8..=127_i8)
        .into_iter()
        .map(|i| if i % 2 == 0 { i } else { -i })
        .take(127)
        .collect::<Vec<i8>>()
);
basic_test!(
    i16,
    vec![0; 40],
    (0..1000)
        .into_iter()
        .map(|i| if i % 2 == 0 { i } else { -i })
        .take(1000)
        .collect::<Vec<i16>>()
);
basic_test!(
    i32,
    vec![0; 40],
    (0..1000)
        .into_iter()
        .map(|i| if i % 2 == 0 { i } else { -i })
        .take(1000)
        .collect::<Vec<i32>>()
);
basic_test!(
    i64,
    vec![0; 40],
    (0..1000)
        .into_iter()
        .map(|i| if i % 2 == 0 { i } else { -i })
        .take(1000)
        .collect::<Vec<i64>>()
);
basic_test!(
    i128,
    vec![0; 40],
    (0..1000)
        .into_iter()
        .map(|i| if i % 2 == 0 { i } else { -i })
        .take(1000)
        .collect::<Vec<i128>>()
);
basic_test!(
    isize,
    vec![0; 40],
    (0..1000)
        .into_iter()
        .map(|i| if i % 2 == 0 { i } else { -i })
        .take(1000)
        .collect::<Vec<isize>>()
);

macro_rules! fixturator_float {
    ( $t:ident ) => {
        fixturator!(
            $t,
            0.0,
            {
                let mut rng = crate::rng();
                if rng.gen() {
                    rng.gen()
                } else {
                    vec![
                        std::$t::NEG_INFINITY,
                        std::$t::INFINITY,
                        std::$t::NAN,
                        -1.0,
                        0.0,
                        1.0,
                    ]
                    .choose(&mut rng)
                    .unwrap()
                    .to_owned()
                }
            },
            {
                let mut index = get_fixt_index!();
                let ret = index as $t;

                let signed_ret = if index % 2 == 0 { ret } else { -ret - 0.5 };
                index += 1;
                set_fixt_index!(index);
                signed_ret
            }
        );
    };
}

fixturator_float!(f32);
fixturator_float!(f64);

basic_test!(
    f32,
    vec![0.0; 40],
    (0_usize..1000)
        .into_iter()
        .map(|u| {
            let f = u as f32;
            if u % 2 == 0 {
                f
            } else {
                -f - 0.5
            }
        })
        .take(1000)
        .collect::<Vec<f32>>()
);
basic_test!(
    f64,
    vec![0.0; 40],
    (0_usize..1000)
        .into_iter()
        .map(|u| {
            let f = u as f64;
            if u % 2 == 0 {
                f
            } else {
                -f - 0.5
            }
        })
        .take(1000)
        .collect::<Vec<f64>>()
);
