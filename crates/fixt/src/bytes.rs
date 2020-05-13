use crate::prelude::*;
use rand::Rng;

const UNPREDICTABLE_MIN_LEN: usize = 0;
const UNPREDICTABLE_MAX_LEN: usize = 32;

type Bytes = Vec<u8>;

fixturator!(
    Bytes,
    vec![],
    {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(UNPREDICTABLE_MIN_LEN, UNPREDICTABLE_MAX_LEN);
        let mut u8_fixturator = U8Fixturator::new(Unpredictable);
        let mut bytes = vec![];
        for _ in 0..len {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    },
    {
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, self.0.index);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        self.0.index = self.0.index + 1;
        bytes
    }
);
