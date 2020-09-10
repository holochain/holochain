//! Fixturators for zome types

use ::fixt::prelude::*;

use crate::entry_def::EntryVisibility;
use crate::header::AppEntryType;
use crate::timestamp::Timestamp;

pub use holo_hash::fixt::*;

fixturator!(
    EntryVisibility;
    unit variants [ Public Private ] empty Public;
);

fixturator!(
    AppEntryType;
    constructor fn new(U8, U8, EntryVisibility);
);

impl Iterator for AppEntryTypeFixturator<EntryVisibility> {
    type Item = AppEntryType;
    fn next(&mut self) -> Option<Self::Item> {
        let app_entry = AppEntryTypeFixturator::new(Unpredictable).next().unwrap();
        Some(AppEntryType::new(
            app_entry.id(),
            app_entry.zome_id(),
            self.0.curve,
        ))
    }
}

fixturator!(
    Timestamp;
    curve Empty (
        Timestamp(I64Fixturator::new(Empty).next().unwrap(), U32Fixturator::new(Empty).next().unwrap())
    );
    curve Unpredictable (
        Timestamp(I64Fixturator::new(Unpredictable).next().unwrap(), U32Fixturator::new(Unpredictable).next().unwrap())
    );
    curve Predictable (
        Timestamp(I64Fixturator::new(Predictable).next().unwrap(), U32Fixturator::new(Predictable).next().unwrap())
    );
);
