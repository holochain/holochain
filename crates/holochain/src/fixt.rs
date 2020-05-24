pub mod curve;

use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContextFixturator;
use fixt::prelude::*;
use holo_hash::AgentPubKeyFixturator;
use holo_hash::DnaHashFixturator;
use holo_hash::HeaderHashFixturator;
use holo_hash::WasmHash;
use holo_hash_core::HeaderHash;
use holochain_types::dna::wasm::DnaWasm;
use holochain_types::dna::zome::Zome;
use holochain_types::dna::DnaDef;
use holochain_types::dna::DnaFile;
use holochain_types::dna::Wasms;
use holochain_types::dna::Zomes;
use holochain_types::test_utils::fake_dna_zomes;
use holochain_wasm_test_utils::strum::IntoEnumIterator;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::capability::CapAccess;
use holochain_zome_types::capability::CapClaim;
use holochain_zome_types::capability::CapGrant;
use holochain_zome_types::capability::CapSecret;
use holochain_zome_types::capability::GrantedFunctions;
use holochain_zome_types::capability::ZomeCallCapGrant;
use holochain_zome_types::header::HeaderHashes;
use holochain_zome_types::migrate_agent::MigrateAgent;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::Entry;
use holochain_zome_types::HostInput;
use rand::seq::IteratorRandom;
use rand::thread_rng;
use rand::Rng;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::Arc;

wasm_io_fixturator!(HostInput<SerializedBytes>);

newtype_fixturator!(ZomeName<String>);

newtype_fixturator!(FnComponents<Vec<String>>);

fixturator!(
    MigrateAgent,
    MigrateAgent::Close,
    {
        if rand::random() {
            MigrateAgent::Close
        } else {
            MigrateAgent::Open
        }
    },
    {
        let ret = if self.0.index % 2 == 0 {
            MigrateAgent::Close
        } else {
            MigrateAgent::Open
        };
        self.0.index = self.0.index + 1;
        ret
    }
);

fixturator!(
    ZomeCallCapGrant,
    {
        match CapGrant::zome_call(
            StringFixturator::new(Empty).next().unwrap(),
            CapAccessFixturator::new(Empty).next().unwrap(),
            {
                let mut rng = rand::thread_rng();
                let number_of_zomes = rng.gen_range(0, 5);

                let mut granted_functions: GrantedFunctions = BTreeMap::new();
                for _ in 0..number_of_zomes {
                    let number_of_functions = rng.gen_range(0, 5);
                    let mut zome_functions = vec![];
                    for _ in 0..number_of_functions {
                        zome_functions.push(StringFixturator::new(Empty).next().unwrap());
                    }
                    granted_functions.insert(
                        ZomeNameFixturator::new(Empty).next().unwrap(),
                        zome_functions,
                    );
                }
                granted_functions
            },
        ) {
            CapGrant::ZomeCall(zome_call) => zome_call,
            _ => unreachable!(),
        }
    },
    {
        match CapGrant::zome_call(
            StringFixturator::new(Unpredictable).next().unwrap(),
            CapAccessFixturator::new(Unpredictable).next().unwrap(),
            {
                let mut rng = rand::thread_rng();
                let number_of_zomes = rng.gen_range(0, 5);

                let mut granted_functions: GrantedFunctions = BTreeMap::new();
                for _ in 0..number_of_zomes {
                    let number_of_functions = rng.gen_range(0, 5);
                    let mut zome_functions = vec![];
                    for _ in 0..number_of_functions {
                        zome_functions.push(StringFixturator::new(Unpredictable).next().unwrap());
                    }
                    granted_functions.insert(
                        ZomeNameFixturator::new(Unpredictable).next().unwrap(),
                        zome_functions,
                    );
                }
                granted_functions
            },
        ) {
            CapGrant::ZomeCall(zome_call) => zome_call,
            _ => unreachable!(),
        }
    },
    {
        let ret = match CapGrant::zome_call(
            StringFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            CapAccessFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            {
                let mut granted_functions: GrantedFunctions = BTreeMap::new();
                for _ in 0..self.0.index % 3 {
                    let number_of_functions = self.0.index % 3;
                    let mut zome_functions = vec![];
                    for _ in 0..number_of_functions {
                        zome_functions.push(StringFixturator::new(Predictable).next().unwrap());
                    }
                    granted_functions.insert(
                        ZomeNameFixturator::new(Predictable).next().unwrap(),
                        zome_functions,
                    );
                }
                granted_functions
            },
        ) {
            CapGrant::ZomeCall(zome_call) => zome_call,
            _ => unreachable!(),
        };
        self.0.index = self.0.index + 1;
        ret
    }
);

fixturator!(
    CapSecret,
    CapSecret::from(StringFixturator::new(Empty).next().unwrap()),
    CapSecret::from(StringFixturator::new(Unpredictable).next().unwrap()),
    CapSecret::from(StringFixturator::new(Predictable).next().unwrap())
);

fixturator!(
    enum CapAccess( Unrestricted, Transferable, Assigned );

    curve Empty {
        match CapAccessIter::random() {
            Unrestricted => CapAccess::unrestricted(),
            Transferable => CapAccess::transferable(),
            Assigned => CapAccess::assigned({
                let mut set = HashSet::new();
                set.insert(fixt!(AgentPubKey, Empty).into());
                set
            })
        }
    };

    curve Unpredictable {

    }
);
curve!(
    CapAccess,
    Empty,

);
curve!(
    CapAccess,
    Unpredictable,

);
curve!(
    CapAccess,
    Predictable,
    {
        match CapAccessIter::indexed(self.0.index) {
            Unrestricted => CapAccess::unrestricted(),
            Transferable => CapAccess::transferable(),
            Assigned => CapAccess::assigned({
                let mut set = HashSet::new();
                set.insert(AgentPubKeyFixturator::new_indexed(Predictable, self.0.index).next().unwrap().into());
                set
            })
        }
    }
);

// #[derive(EnumIter)]
// enum CapAccessEnumEnum {
//     Unrestricted,
//     Transferable,
//     Assigned,
// }
//
// impl From<CapAccess> for CapAccessEnumEnum {
//     fn from(cap_access: CapAccess) -> Self {
//         match cap_access {
//             CapAccess::Unrestricted => Self::Unrestricted,
//             CapAccess::Transferable { .. } => Self::Transferable,
//             CapAccess::Assigned { .. } => Self::Assigned,
//         }
//     }
// }
//
// fixturator!(
//     CapAccess,
//     {
//         match CapAccessEnumEnum::iter().choose(&mut thread_rng()).unwrap() {
//             CapAccessEnumEnum::Unrestricted => CapAccess::unrestricted(),
//             CapAccessEnumEnum::Transferable => CapAccess::transferable(),
//             CapAccessEnumEnum::Assigned => CapAccess::assigned({
//                 let mut set = HashSet::new();
//                 set.insert(AgentPubKeyFixturator::new(Empty).next().unwrap().into());
//                 set
//             }),
//         }
//     },
//     {
//         match CapAccessEnumEnum::iter().choose(&mut thread_rng()).unwrap() {
//             CapAccessEnumEnum::Unrestricted => CapAccess::unrestricted(),
//             CapAccessEnumEnum::Transferable => CapAccess::transferable(),
//             CapAccessEnumEnum::Assigned => CapAccess::assigned({
//                 let mut set = HashSet::new();
//                 set.insert(
//                     AgentPubKeyFixturator::new(Unpredictable)
//                         .next()
//                         .unwrap()
//                         .into(),
//                 );
//                 set
//             }),
//         }
//     },
//     {
//         let ret = match CapAccessEnumEnum::iter().cycle().nth(self.0.index).unwrap() {
//             CapAccessEnumEnum::Unrestricted => CapAccess::unrestricted(),
//             CapAccessEnumEnum::Transferable => CapAccess::transferable(),
//             CapAccessEnumEnum::Assigned => CapAccess::assigned({
//                 let mut set = HashSet::new();
//                 set.insert(
//                     AgentPubKeyFixturator::new_indexed(Predictable, self.0.index)
//                         .next()
//                         .unwrap()
//                         .into(),
//                 );
//                 set
//             }),
//         };
//         self.0.index = self.0.index + 1;
//         ret
//     }
// );

/// dummy to allow us to randomly select a cap grant variant inside the fixturator
#[derive(EnumIter)]
enum CapGrantEnumEnum {
    Authorship,
    ZomeCall,
}

/// never do this
/// tricks the compiler into complaining about variant mismatch via the match
impl From<CapGrant> for CapGrantEnumEnum {
    fn from(cap_grant: CapGrant) -> Self {
        match cap_grant {
            CapGrant::Authorship(_) => Self::Authorship,
            CapGrant::ZomeCall(_) => Self::ZomeCall,
        }
    }
}

fixturator!(
    CapGrant,
    {
        match CapGrantEnumEnum::iter().choose(&mut thread_rng()).unwrap() {
            CapGrantEnumEnum::Authorship => {
                CapGrant::Authorship(AgentPubKeyFixturator::new(Empty).next().unwrap().into())
            }
            CapGrantEnumEnum::ZomeCall => {
                CapGrant::ZomeCall(ZomeCallCapGrantFixturator::new(Empty).next().unwrap())
            }
        }
    },
    {
        match CapGrantEnumEnum::iter().choose(&mut thread_rng()).unwrap() {
            CapGrantEnumEnum::Authorship => CapGrant::Authorship(
                AgentPubKeyFixturator::new(Unpredictable)
                    .next()
                    .unwrap()
                    .into(),
            ),
            CapGrantEnumEnum::ZomeCall => CapGrant::ZomeCall(
                ZomeCallCapGrantFixturator::new(Unpredictable)
                    .next()
                    .unwrap(),
            ),
        }
    },
    {
        let ret = match CapGrantEnumEnum::iter().cycle().nth(self.0.index).unwrap() {
            CapGrantEnumEnum::Authorship => CapGrant::Authorship(
                AgentPubKeyFixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap()
                    .into(),
            ),
            CapGrantEnumEnum::ZomeCall => CapGrant::ZomeCall(
                ZomeCallCapGrantFixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap(),
            ),
        };
        self.0.index = self.0.index + 1;
        ret
    }
);

fixturator!(
    CapClaim,
    CapClaim::new(
        StringFixturator::new(Empty).next().unwrap(),
        AgentPubKeyFixturator::new(Empty).next().unwrap().into(),
        CapSecretFixturator::new(Empty).next().unwrap()
    ),
    CapClaim::new(
        StringFixturator::new(Unpredictable).next().unwrap(),
        AgentPubKeyFixturator::new(Unpredictable)
            .next()
            .unwrap()
            .into(),
        CapSecretFixturator::new(Unpredictable).next().unwrap(),
    ),
    {
        let ret = CapClaim::new(
            StringFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            AgentPubKeyFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap()
                .into(),
            CapSecretFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        );
        self.0.index = self.0.index + 1;
        ret
    }
);

/// dummy to let us randomly select an entry variant inside the fixturator
#[derive(EnumIter)]
enum EntryEnumEnum {
    Agent,
    App,
    CapClaim,
    CapGrant,
}

/// never do this
/// this exists to trick the compiler into complaining if the enum variants ever fall out of sync
/// due to the inner match
impl From<Entry> for EntryEnumEnum {
    fn from(entry: Entry) -> Self {
        match entry {
            Entry::Agent(_) => EntryEnumEnum::Agent,
            Entry::App(_) => EntryEnumEnum::App,
            Entry::CapClaim(_) => EntryEnumEnum::CapClaim,
            Entry::CapGrant(_) => EntryEnumEnum::CapGrant,
        }
    }
}

// enum_fixturator!(Entry::Agent(AgentPubKey), Entry::App(SerializedBytes), Entry::CapClaim(CapClaim), Entry::CapGrant(ZomeCallCapGrant));

// {
//     #[derive(EnumIter)]
//     <$enum>FixturatorEnum {
//         $( $variant ),*
//     }
//
//     fixturator!(
//         $enum,
//         {
//             match <$enum>FixturatorEnum::iter().choose(&mut thread_rng()).unwrap() {
//                 <$enum>FixturatorEnum::Agent => {
//                     Entry::Agent(AgentPubKeyFixturator::new(Empty).next().unwrap().into())
//                 }
//                 <$enum>FixturatorEnum::App => Entry::App(SerializedBytesFixturator::new(Empty).next().unwrap()),
//                 <$enum>FixturatorEnum::CapClaim => {
//                     Entry::CapClaim(CapClaimFixturator::new(Empty).next().unwrap())
//                 }
//                 <$enum>FixturatorEnum::CapGrant => {
//                     Entry::CapGrant(ZomeCallCapGrantFixturator::new(Empty).next().unwrap())
//                 }
//             }
//         },
// }

fixturator!(
    Entry,
    {
        match EntryEnumEnum::iter().choose(&mut thread_rng()).unwrap() {
            EntryEnumEnum::Agent => {
                Entry::Agent(AgentPubKeyFixturator::new(Empty).next().unwrap().into())
            }
            EntryEnumEnum::App => Entry::App(SerializedBytesFixturator::new(Empty).next().unwrap()),
            EntryEnumEnum::CapClaim => {
                Entry::CapClaim(CapClaimFixturator::new(Empty).next().unwrap())
            }
            EntryEnumEnum::CapGrant => {
                Entry::CapGrant(ZomeCallCapGrantFixturator::new(Empty).next().unwrap())
            }
        }
    },
    {
        match EntryEnumEnum::iter().choose(&mut thread_rng()).unwrap() {
            EntryEnumEnum::Agent => Entry::Agent(
                AgentPubKeyFixturator::new(Unpredictable)
                    .next()
                    .unwrap()
                    .into(),
            ),
            EntryEnumEnum::App => Entry::App(
                SerializedBytesFixturator::new(Unpredictable)
                    .next()
                    .unwrap(),
            ),
            EntryEnumEnum::CapClaim => {
                Entry::CapClaim(CapClaimFixturator::new(Unpredictable).next().unwrap())
            }
            EntryEnumEnum::CapGrant => Entry::CapGrant(
                ZomeCallCapGrantFixturator::new(Unpredictable)
                    .next()
                    .unwrap(),
            ),
        }
    },
    {
        let ret = match EntryEnumEnum::iter().cycle().nth(self.0.index).unwrap() {
            EntryEnumEnum::Agent => Entry::Agent(
                AgentPubKeyFixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap()
                    .into(),
            ),
            EntryEnumEnum::App => Entry::App(
                SerializedBytesFixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap(),
            ),
            EntryEnumEnum::CapClaim => Entry::CapClaim(
                CapClaimFixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap(),
            ),
            EntryEnumEnum::CapGrant => Entry::CapGrant(
                ZomeCallCapGrantFixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap(),
            ),
        };
        self.0.index = self.0.index + 1;
        ret
    }
);

fixturator!(
    HeaderHashes,
    vec![].into(),
    {
        let mut rng = rand::thread_rng();
        let number_of_hashes = rng.gen_range(0, 5);

        let mut hashes: Vec<HeaderHash> = vec![];
        let mut header_hash_fixturator = HeaderHashFixturator::new(Unpredictable);
        for _ in (0..number_of_hashes) {
            hashes.push(header_hash_fixturator.next().unwrap().into());
        }
        hashes.into()
    },
    {
        let mut hashes: Vec<HeaderHash> = vec![];
        let mut header_hash_fixturator =
            HeaderHashFixturator::new_indexed(Predictable, self.0.index);
        for _ in 0..3 {
            hashes.push(header_hash_fixturator.next().unwrap().into());
        }
        self.0.index = self.0.index + 1;
        hashes.into()
    }
);

fixturator!(
    Wasms,
    { BTreeMap::new() },
    {
        let mut rng = rand::thread_rng();
        let number_of_wasms = rng.gen_range(0, 5);

        let mut wasms: Wasms = BTreeMap::new();
        let mut dna_wasm_fixturator = DnaWasmFixturator::new(Unpredictable);
        for _ in (0..number_of_wasms) {
            let wasm = dna_wasm_fixturator.next().unwrap();
            wasms.insert(
                tokio_safe_block_on::tokio_safe_block_on(
                    async { WasmHash::with_data(&*wasm.code()).await },
                    std::time::Duration::from_millis(10),
                )
                .unwrap()
                .into(),
                wasm,
            );
        }
        wasms
    },
    {
        let mut wasms: Wasms = BTreeMap::new();
        let mut dna_wasm_fixturator = DnaWasmFixturator::new_indexed(Predictable, self.0.index);
        for _ in (0..3) {
            let wasm = dna_wasm_fixturator.next().unwrap();
            wasms.insert(
                tokio_safe_block_on::tokio_safe_block_on(
                    async { WasmHash::with_data(&*wasm.code()).await },
                    std::time::Duration::from_millis(10),
                )
                .unwrap()
                .into(),
                wasm,
            );
        }
        self.0.index = self.0.index + 1;
        wasms
    }
);

fixturator!(
    Zomes,
    { Vec::new() },
    {
        // @todo implement unpredictable zomes
        ZomesFixturator::new(Empty).next().unwrap()
    },
    {
        // @todo implement predictable zomes
        ZomesFixturator::new(Empty).next().unwrap()
    }
);

fixturator!(
    DnaWasm,
    {
        // note that an empty wasm will not compile
        let code = vec![];
        DnaWasm {
            code: Arc::new(code),
        }
    },
    {
        let mut rng = thread_rng();
        TestWasm::iter().choose(&mut rng).unwrap().into()
    },
    {
        let wasm = TestWasm::iter().cycle().nth(self.0.index).unwrap();
        self.0.index = self.0.index + 1;
        wasm.into()
    }
);

fixturator!(
    DnaDef,
    {
        let dna_def = DnaDef {
            name: StringFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
            uuid: StringFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
            properties: SerializedBytesFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
            zomes: ZomesFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        dna_def
    },
    {
        let dna_def = DnaDef {
            name: StringFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
            uuid: StringFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
            properties: SerializedBytesFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
            zomes: ZomesFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        dna_def
    },
    {
        let dna_def = DnaDef {
            name: StringFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            uuid: StringFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            properties: SerializedBytesFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            zomes: ZomesFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        dna_def
    }
);

fixturator!(
    DnaFile,
    {
        DnaFile {
            dna: DnaDefFixturator::new(Empty).next().unwrap(),
            dna_hash: DnaHashFixturator::new(Empty).next().unwrap(),
            code: WasmsFixturator::new(Empty).next().unwrap(),
        }
    },
    {
        // align the wasm hashes across the file and def
        let mut zome_name_fixturator = ZomeNameFixturator::new(Unpredictable);
        let wasms = WasmsFixturator::new(Unpredictable).next().unwrap();
        let mut zomes: Zomes = Vec::new();
        for (hash, wasm) in wasms {
            zomes.push((
                zome_name_fixturator.next().unwrap(),
                Zome {
                    wasm_hash: hash.to_owned(),
                },
            ));
        }
        let mut dna_def = DnaDefFixturator::new(Unpredictable).next().unwrap();
        dna_def.zomes = zomes;
        DnaFile {
            dna: dna_def,
            dna_hash: DnaHashFixturator::new(Unpredictable).next().unwrap(),
            code: WasmsFixturator::new(Unpredictable).next().unwrap(),
        }
    },
    {
        // align the wasm hashes across the file and def
        let mut zome_name_fixturator = ZomeNameFixturator::new_indexed(Predictable, self.0.index);
        let wasms = WasmsFixturator::new_indexed(Predictable, self.0.index)
            .next()
            .unwrap();
        let mut zomes: Zomes = Vec::new();
        for (hash, wasm) in wasms {
            zomes.push((
                zome_name_fixturator.next().unwrap(),
                Zome {
                    wasm_hash: hash.to_owned(),
                },
            ));
        }
        let mut dna_def = DnaDefFixturator::new_indexed(Predictable, self.0.index)
            .next()
            .unwrap();
        dna_def.zomes = zomes;
        let dna_file = DnaFile {
            dna: DnaDefFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            dna_hash: DnaHashFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            code: WasmsFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        dna_file
    }
);

fixturator!(
    WasmRibosome,
    {
        WasmRibosome {
            dna_file: DnaFileFixturator::new(Empty).next().unwrap(),
        }
    },
    {
        WasmRibosome {
            dna_file: DnaFileFixturator::new(Unpredictable).next().unwrap(),
        }
    },
    {
        let ribosome = WasmRibosome {
            dna_file: DnaFileFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        ribosome
    }
);

impl Iterator for WasmRibosomeFixturator<curve::Zomes> {
    type Item = WasmRibosome;

    fn next(&mut self) -> Option<Self::Item> {
        // @todo fixturate this
        let dna_file = fake_dna_zomes(
            &StringFixturator::new(Unpredictable).next().unwrap(),
            self.0
                .curve
                .0
                .clone()
                .into_iter()
                .map(|t| (t.into(), t.into()))
                .collect(),
        );
        let ribosome = WasmRibosome::new(dna_file);

        // warm the module cache for each wasm in the ribosome
        for zome in self.0.curve.0.clone() {
            let mut host_context = HostContextFixturator::new(Empty).next().unwrap();
            host_context.zome_name = zome.into();
            ribosome.module(host_context).unwrap();
        }

        self.0.index = self.0.index + 1;

        Some(ribosome)
    }
}
