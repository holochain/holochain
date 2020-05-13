pub mod curve;

use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContextFixturator;
use fixt::prelude::*;
use holo_hash::DnaHashFixturator;
use holo_hash::WasmHash;
use holochain_types::dna::wasm::DnaWasm;
use holochain_types::dna::zome::Zome;
use holochain_types::dna::DnaDef;
use holochain_types::dna::DnaFile;
use holochain_types::dna::Wasms;
use holochain_types::dna::Zomes;
use holochain_types::test_utils::fake_dna_zomes;
use holochain_wasm_test_utils::strum::IntoEnumIterator;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::migrate_agent::MigrateAgent;
use holochain_zome_types::zome::ZomeName;
use rand::seq::IteratorRandom;
use rand::thread_rng;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::Arc;

newtype_fixturator!(ZomeName<String>);
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
    Wasms,
    { BTreeMap::new() },
    {
        let mut rng = rand::thread_rng();
        let number_of_wasms = rng.gen_range(0, 5);

        let mut wasms: Wasms = BTreeMap::new();
        let mut dna_wasm_fixturator = DnaWasmFixturator::new(Unpredictable);
        for _ in (0..number_of_wasms) {
            let wasm = dna_wasm_fixturator.next().unwrap();
            wasms.insert(WasmHash::with_data_sync(&*wasm.code()).into(), wasm);
        }
        wasms
    },
    {
        let mut wasms: Wasms = BTreeMap::new();
        let mut dna_wasm_fixturator = DnaWasmFixturator::new_indexed(Predictable, self.0.index);
        for _ in (0..3) {
            let wasm = dna_wasm_fixturator.next().unwrap();
            wasms.insert(WasmHash::with_data_sync(&*wasm.code()).into(), wasm);
        }
        self.0.index = self.0.index + 1;
        wasms
    }
);

fixturator!(
    Zomes,
    { BTreeMap::new() },
    {
        // @todo implement unpredictable zomes
        BTreeMap::new()
    },
    {
        // @todo implement predictable zomes
        BTreeMap::new()
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
        let mut zomes: Zomes = BTreeMap::new();
        for (hash, wasm) in wasms {
            zomes.insert(
                zome_name_fixturator.next().unwrap(),
                Zome {
                    wasm_hash: hash.to_owned(),
                },
            );
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
        let mut zomes: Zomes = BTreeMap::new();
        for (hash, wasm) in wasms {
            zomes.insert(
                zome_name_fixturator.next().unwrap(),
                Zome {
                    wasm_hash: hash.to_owned(),
                },
            );
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
