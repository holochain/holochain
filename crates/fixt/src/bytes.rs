use crate::prelude::*;
use rand::Rng;

const UNPREDICTABLE_MIN_LEN: usize = 0;
const UNPREDICTABLE_MAX_LEN: usize = 32;

pub type Bytes = Vec<u8>;
pub type BytesNotEmpty = Vec<u8>;

// Simply generate "bytes" which is a Vec<u8>
// likely the most interesting is the Unpredictable curve that throws out random bytes in a vec
// of random length between 0 and 32 bytes long
fixturator!(
    Bytes;
    curve Empty vec![];
    curve Unpredictable {
        let mut rng = crate::rng();
        let len = rng.gen_range(UNPREDICTABLE_MIN_LEN, UNPREDICTABLE_MAX_LEN);
        let mut u8_fixturator = U8Fixturator::new(Unpredictable);
        let mut bytes = vec![];
        for _ in 0..len {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    };
    curve Predictable {
        let mut index = get_fixt_index!();
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, index);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        index += 1;
        set_fixt_index!(index);
        bytes
    };
);

// Simply generate "bytes" which is a Vec<u8>
// likely the most interesting is the Unpredictable curve that throws out random bytes in a vec
// of random length between 1 and 32 bytes long
// This version of Bytes is never empty.
fixturator!(
    BytesNotEmpty;
    curve Empty vec![0u8];
    curve Unpredictable {
        let mut rng = crate::rng();
        let len = rng.gen_range(1, UNPREDICTABLE_MAX_LEN);
        let mut u8_fixturator = U8Fixturator::new(Unpredictable);
        let mut bytes = vec![];
        for _ in 0..len {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    };
    curve Predictable {
        let mut index = get_fixt_index!();
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, index);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        index += 1;
        set_fixt_index!(index);
        bytes
    };
);

/// A type alias for a Vec<u8> whose fixturator is expected to only return
/// a Vec of length 36
pub type ThirtySixBytes = Vec<u8>;

// Simply generate "bytes" which is a Vec<u8> of 36 bytes
fixturator!(
    ThirtySixBytes;
    curve Empty [0; 36].to_vec();
    curve Predictable {
        let mut u8_fixturator = U8Fixturator::new(Predictable);
        let mut bytes = vec![];
        for _ in 0..36 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    };
    curve Unpredictable {
        let mut u8_fixturator = U8Fixturator::new_indexed(Unpredictable, get_fixt_index!());
        let mut bytes = vec![];
        for _ in 0..36 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    };
);

/// A type alias for a Vec<u8> whose fixturator is expected to only return
/// a Vec of length 32
pub type ThirtyTwoBytes = [u8; 32];

// Simply generate "bytes" which is a Vec<u8> of 32 bytes
fixturator!(
    ThirtyTwoBytes;
    curve Empty [0; 32];
    curve Unpredictable {
        rand::thread_rng().gen::<[u8; 32]>()
    };
    curve Predictable {
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, get_fixt_index!());
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        let mut ret = [0; 32];
        ret.copy_from_slice(&bytes);
        ret
    };
);

/// A type alias for a Vec<u8> whose fixturator is expected to only return
/// a Vec of length 64
pub type SixtyFourBytes = [u8; 64];

// Simply generate "bytes" which is a Vec<u8> of 32 bytes
fixturator!(
    SixtyFourBytes;
    curve Empty [0; 64];
    curve Unpredictable {
        let bytes: Vec<u8> = (0..64).map(|_| rand::random::<u8>()).collect();
        let mut ret = [0; 64];
        ret.copy_from_slice(&bytes);
        ret
    };
    curve Predictable {
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, get_fixt_index!());
        let mut bytes = vec![];
        for _ in 0..64 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        let mut ret = [0; 64];
        ret.copy_from_slice(&bytes);
        ret
    };
);

pub type SixtyFourBytesVec = Vec<u8>;

fixturator!(
    SixtyFourBytesVec;
    curve Empty [0; 64].to_vec();
    curve Unpredictable {
        SixtyFourBytesFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap().to_vec()
    };
    curve Predictable {
        SixtyFourBytesFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap().to_vec()
    };
);
