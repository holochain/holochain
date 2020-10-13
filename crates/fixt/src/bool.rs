use crate::prelude::*;
use crate::FIXTURATOR_RNG;

fixturator!(bool, false, FIXTURATOR_RNG.lock().gen(), {
    self.0.index += 1;
    self.0.index % 2 != 0
});

basic_test!(
    bool,
    vec![false; 40],
    vec![true, false]
        .into_iter()
        .cycle()
        .take(20)
        .collect::<Vec<bool>>()
);
