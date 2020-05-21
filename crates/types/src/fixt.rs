//! Fixture definitions for holochain_types structs

// FIXME (aka fixtme, haha, get it?) move other fixturators from this crate into this module

use crate::header::{AppEntryType, EntryVisibility};
use fixt::prelude::*;
use holochain_zome_types::capability::CapSecret;
use rand;

// This technically belongs in holochain_zome_types, but we want to keep the size down
fixturator!(
    CapSecret,
    CapSecret::from(StringFixturator::new(Empty).next().unwrap()),
    CapSecret::from(StringFixturator::new(Unpredictable).next().unwrap()),
    {
        let v = CapSecret::from(
            StringFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        );
        self.0.index = self.0.index + 1;
        v
    }
);

fixturator!(
    EntryVisibility,
    EntryVisibility::Public,
    {
        if rand::random() {
            EntryVisibility::Public
        } else {
            EntryVisibility::Private
        }
    },
    {
        let v = if self.0.index % 2 == 0 {
            EntryVisibility::Private
        } else {
            EntryVisibility::Public
        };
        self.0.index += 1;
        v
    }
);

fixturator!(
    AppEntryType,
    AppEntryType {
        id: BytesFixturator::new(Empty).next().unwrap(),
        zome_id: U8Fixturator::new(Empty).next().unwrap(),
        visibility: EntryVisibilityFixturator::new(Empty).next().unwrap(),
    },
    AppEntryType {
        id: BytesFixturator::new(Unpredictable).next().unwrap(),
        zome_id: U8Fixturator::new(Unpredictable).next().unwrap(),
        visibility: EntryVisibilityFixturator::new(Unpredictable)
            .next()
            .unwrap(),
    },
    {
        let app_entry_type = AppEntryType {
            id: BytesFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            zome_id: U8Fixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            visibility: EntryVisibilityFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        app_entry_type
    }
);
