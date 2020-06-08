use crate::prelude::*;
use rand::Rng;

const UNPREDICTABLE_MIN_LEN: usize = 0;
const UNPREDICTABLE_MAX_LEN: usize = 32;

type Bytes = Vec<u8>;

// Simply generate "bytes" which is a Vec<u8>
// likely the most interesting is the Unpredictable curve that throws out random bytes in a vec
// of random length between 0 and 32 bytes long
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
        self.0.index += 1;
        bytes
    }
);
