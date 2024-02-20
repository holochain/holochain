use crate::KitsuneAgent;
use crate::KitsuneBinType;
use crate::KitsuneOpHash;
use crate::KitsuneSignature;
use crate::KitsuneSpace;
use ::fixt::prelude::*;

fixturator!(
    KitsuneAgent;
    constructor fn new(ThirtySixBytes);
);

fixturator!(
    KitsuneSpace;
    constructor fn new(ThirtySixBytes);
);

fixturator!(
    KitsuneOpHash;
    constructor fn new(ThirtySixBytes);
);

fixturator!(
    KitsuneSignature;
    from SixtyFourBytesVec;
);
