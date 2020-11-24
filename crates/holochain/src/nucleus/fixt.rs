use super::dna::zome::HostFnAccess;
use super::dna::zome::Permission;
use super::dna::zome::Zome;
use super::dna::zome::ZomeDef;
use super::dna::DnaDef;
use super::dna::Zomes;
use ::fixt::prelude::*;
use holochain_types::fixt::*;

fixturator!(
    Zome;
    constructor fn new(ZomeName, ZomeDef);
);

fixturator!(
    ZomeDef;
    constructor fn from_hash(WasmHash);
);

fixturator!(
    Zomes;
    curve Empty Vec::new();
    curve Unpredictable {
        // @todo implement unpredictable zomes
        ZomesFixturator::new(Empty).next().unwrap()
    };
    curve Predictable {
        // @todo implement predictable zomes
        ZomesFixturator::new(Empty).next().unwrap()
    };
);

fixturator!(
    DnaDef;
    curve Empty DnaDef {
        name: StringFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        uuid: StringFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        zomes: ZomesFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
    };

    curve Unpredictable DnaDef {
        name: StringFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        uuid: StringFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        zomes: ZomesFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
    };

    curve Predictable DnaDef {
        name: StringFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        uuid: StringFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        zomes: ZomesFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
    };
);

fixturator!(
    Permission;
    unit variants [ Allow Deny ] empty Deny;
);

fixturator!(
    HostFnAccess;
    constructor fn new(Permission, Permission, Permission, Permission, Permission, Permission, Permission);
);
