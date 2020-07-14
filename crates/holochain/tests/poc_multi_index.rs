use holo_hash::DhtOpHash;
use holo_hash_core::HoloHashCoreHash;
use holochain_types::Timestamp;
use rand::Rng;
use std::collections::{BTreeSet, HashMap};

/// This would be the "Value" type stored in the intergrated_dht_ops store.
/// The store is keyd by DhtOpHash optimized for random lookup.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct FakeIntegratedDhtOpEntry {
    dht_op_hash: DhtOpHash,
    integrated_at: Timestamp,
    // also the actual DhtOp : )
}

/// the Dht "location" type (the real version uses wrapping types...
/// we're just doing the wrapping manually inline below)
pub type Location = u32;

/// The fast lookup index holds only recently integrated entries
/// The fast loop doesn't do time filtering, just returns entries within dht arc
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FastLoopIndexKey(Location, Timestamp, DhtOpHash);

/// The longtail consistency loop requires querying by dht arc AND time range
/// We'll need to do experiments with real actual data / usecases to determine
/// whether DhtArc or Timestamp range should come first in the index,
/// but I think it'll work decently efficiently to start with the DhtArc first.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LongConsistencyLoopIndexKey(Location, Timestamp, DhtOpHash);

/// Since we're starting with DhtArc, we need to be able to min/max the rest
/// of the keys including timestamp
const MIN_TIMESTAMP: Timestamp = Timestamp(i64::MIN, u32::MIN);
const MAX_TIMESTAMP: Timestamp = Timestamp(i64::MAX, u32::MAX);

// Since we're starting with DhtArc, we need to be able to min/max the rest
// of the keys including dht op hash
lazy_static::lazy_static! {
    static ref MIN_DHT_OP_HASH: DhtOpHash = {
        DhtOpHash::from(holo_hash_core::DhtOpHash::new(vec![0; 36]))
    };

    static ref MAX_DHT_OP_HASH: DhtOpHash = {
        DhtOpHash::from(holo_hash_core::DhtOpHash::new(vec![0xff; 36]))
    };
}

/// Generate a random IntegratedDhtOpEntry
/// The dht_op_hash is completely random (and hence also the dht location)
/// The integrated_at is random from 2 hours ago up until now.
fn gen_random_entry() -> FakeIntegratedDhtOpEntry {
    let mut dht_op_hash = vec![0; 36];
    rand::thread_rng().fill(&mut dht_op_hash[..]);
    let dht_op_hash = DhtOpHash::from(holo_hash_core::DhtOpHash::new(dht_op_hash));
    let mut integrated_at = Timestamp::now();
    integrated_at.0 /* secs */ -= rand::thread_rng().gen_range(0, 60 * 60 * 2);
    FakeIntegratedDhtOpEntry {
        dht_op_hash,
        integrated_at,
    }
}

/// this is the dht arc structure for querying the store
/// the "center_loc" is the basis loc of the entry
/// the "half_length" is how far on both sides the arc extends
pub struct DhtArc {
    center_loc: u32,
    half_length: i32,
}

/// This would be LMDB - but right now just testing out with native rust types
pub struct FakeIntegratedDhtOpStore {
    /// the actual random-access (hash) optimized full store
    integrated_dht_op_store: HashMap<DhtOpHash, FakeIntegratedDhtOpEntry>,
    /// the dht_loc sort-optimized fast loop index
    fast_loop_index: BTreeSet<FastLoopIndexKey>,
    /// the dht_loc sort-optimized longtail consistency loop index
    long_consistency_loop_index: BTreeSet<LongConsistencyLoopIndexKey>,
}

impl FakeIntegratedDhtOpStore {
    /// create a new fake store
    pub fn new() -> Self {
        Self {
            integrated_dht_op_store: HashMap::new(),
            fast_loop_index: BTreeSet::new(),
            long_consistency_loop_index: BTreeSet::new(),
        }
    }

    /// insert a new fake entry into the fake store
    pub fn insert(&mut self, entry: FakeIntegratedDhtOpEntry) {
        let mut recent = Timestamp::now();
        recent.0 /* secs */ -= 60 * 60;

        // if this entry is "recent" (within the last hour)
        // add it to the fast_loop index
        if entry.integrated_at > recent {
            self.fast_loop_index.insert(FastLoopIndexKey(
                // TODO - INCORRECT!!
                //        this needs to be the loc of the basis hash
                entry.dht_op_hash.get_loc(),
                entry.integrated_at.clone(),
                entry.dht_op_hash.clone(),
            ));
        }

        // always add entries to the longtail consistency loop index
        self.long_consistency_loop_index
            .insert(LongConsistencyLoopIndexKey(
                // TODO - INCORRECT!!
                //        this needs to be the loc of the basis hash
                entry.dht_op_hash.get_loc(),
                entry.integrated_at.clone(),
                entry.dht_op_hash.clone(),
            ));

        // always add entries to the actual store
        self.integrated_dht_op_store
            .insert(entry.dht_op_hash.clone(), entry);
    }

    /// periodically call this to age things out of the fast loop index
    pub fn purge_fast_loop(&mut self) {
        let mut recent = Timestamp::now();
        recent.0 /* secs */ -= 60 * 60;

        // would be nice if drain_filter was stable... : )

        let old_set = std::mem::replace(&mut self.fast_loop_index, BTreeSet::new());

        for i in old_set.into_iter() {
            if i.1 > recent {
                self.fast_loop_index.insert(i);
            }
        }
    }

    // for longtail consistency we always query within a dht arc
    // and within a time range
    pub fn query_long_consistency(
        &self,
        dht_arc: DhtArc,
        from: Timestamp,
        until: Timestamp,
    ) -> Vec<DhtOpHash> {
        // first off - if the half_len is negative, nothing will match
        if dht_arc.half_length < 0 {
            return vec![];
        }

        // figure out our linear start location (using wrapping subtract)
        let start = (std::num::Wrapping(dht_arc.center_loc)
            - std::num::Wrapping(dht_arc.half_length as u32))
        .0;

        // total len
        let len: u32 = dht_arc.half_length as u32 * 2;

        // figure out the linear end location (using wrapping add)
        let mut end = (std::num::Wrapping(start) + std::num::Wrapping(len)).0;

        let mut out = Vec::new();

        // if we wrapped (i.e. the end is before the start),
        // we need to do this in two stages
        if end < start {
            // find all matches from MIN location to the end location
            for entry in self.long_consistency_loop_index.range((
                std::ops::Bound::Included(LongConsistencyLoopIndexKey(
                    u32::MIN,
                    MIN_TIMESTAMP,
                    MIN_DHT_OP_HASH.clone(),
                )),
                std::ops::Bound::Included(LongConsistencyLoopIndexKey(
                    end,
                    MAX_TIMESTAMP,
                    MAX_DHT_OP_HASH.clone(),
                )),
            )) {
                // accept only timestamps that match given range
                if entry.1 > from && entry.1 < until {
                    out.push(entry.2.clone());
                }
            }

            // reset end to be MAX so start to MAX will be the other half
            // of the range
            end = u32::MAX;
        }

        for entry in self.long_consistency_loop_index.range((
            std::ops::Bound::Included(LongConsistencyLoopIndexKey(
                start,
                MIN_TIMESTAMP,
                MIN_DHT_OP_HASH.clone(),
            )),
            std::ops::Bound::Included(LongConsistencyLoopIndexKey(
                end,
                MAX_TIMESTAMP,
                MAX_DHT_OP_HASH.clone(),
            )),
        )) {
            // accept only timestamps that match given range
            if entry.1 > from && entry.1 < until {
                out.push(entry.2.clone());
            }
        }

        out
    }

    // for the fast loop, we just get everything within the DhtArc
    // as things age out of the fast index they will no longer be returned
    pub fn query_fast_loop(&self, dht_arc: DhtArc) -> Vec<DhtOpHash> {
        // first off - if the half_len is negative, nothing will match
        if dht_arc.half_length < 0 {
            return vec![];
        }

        // figure out our linear start location (using wrapping subtract)
        let start = (std::num::Wrapping(dht_arc.center_loc)
            - std::num::Wrapping(dht_arc.half_length as u32))
        .0;

        // total len
        let len: u32 = dht_arc.half_length as u32 * 2;

        // figure out the linear end location (using wrapping add)
        let mut end = (std::num::Wrapping(start) + std::num::Wrapping(len)).0;

        let mut out = Vec::new();

        // if we wrapped (i.e. the end is before the start),
        // we need to do this in two stages
        if end < start {
            // find all matches from MIN location to the end location
            for entry in self.fast_loop_index.range((
                std::ops::Bound::Included(FastLoopIndexKey(
                    u32::MIN,
                    MIN_TIMESTAMP,
                    MIN_DHT_OP_HASH.clone(),
                )),
                std::ops::Bound::Included(FastLoopIndexKey(
                    end,
                    MAX_TIMESTAMP,
                    MAX_DHT_OP_HASH.clone(),
                )),
            )) {
                out.push(entry.2.clone());
            }

            // reset end to be MAX so start to MAX will be the other half
            // of the range
            end = u32::MAX;
        }

        for entry in self.fast_loop_index.range((
            std::ops::Bound::Included(FastLoopIndexKey(
                start,
                MIN_TIMESTAMP,
                MIN_DHT_OP_HASH.clone(),
            )),
            std::ops::Bound::Included(FastLoopIndexKey(
                end,
                MAX_TIMESTAMP,
                MAX_DHT_OP_HASH.clone(),
            )),
        )) {
            out.push(entry.2.clone());
        }

        out
    }
}

#[test]
fn poc_multi_index() {
    let mut store = FakeIntegratedDhtOpStore::new();

    for _ in 0..100 {
        store.insert(gen_random_entry());
    }

    // just prove out purge doesn't break anything
    store.purge_fast_loop();

    println!(
        "should get all 100 (full wrap): count: {}",
        store
            .query_long_consistency(
                DhtArc {
                    center_loc: u32::MAX,
                    half_length: (u32::MAX / 2) as i32,
                },
                MIN_TIMESTAMP,
                MAX_TIMESTAMP,
            )
            .len()
    );

    println!(
        "should get all 100 (start): count: {}",
        store
            .query_long_consistency(
                DhtArc {
                    center_loc: u32::MIN,
                    half_length: (u32::MAX / 2) as i32,
                },
                MIN_TIMESTAMP,
                MAX_TIMESTAMP,
            )
            .len()
    );

    println!(
        "should get all 100 (midpoint): count: {}",
        store
            .query_long_consistency(
                DhtArc {
                    center_loc: u32::MAX / 2,
                    half_length: (u32::MAX / 2) as i32,
                },
                MIN_TIMESTAMP,
                MAX_TIMESTAMP,
            )
            .len()
    );

    println!(
        "should get ~ 50 (1/4 half len): count: {}",
        store
            .query_long_consistency(
                DhtArc {
                    center_loc: u32::MAX / 2,
                    half_length: (u32::MAX / 4) as i32,
                },
                MIN_TIMESTAMP,
                MAX_TIMESTAMP,
            )
            .len()
    );

    let mut recent = Timestamp::now();
    recent.0 /* secs */ -= 60 * 60;
    println!(
        "should get ~ 50 (time): count: {}",
        store
            .query_long_consistency(
                DhtArc {
                    center_loc: u32::MAX / 2,
                    half_length: (u32::MAX / 2) as i32,
                },
                recent,
                MAX_TIMESTAMP,
            )
            .len()
    );

    println!(
        "should get ~ 50 (fast_loop): count: {}",
        store
            .query_fast_loop(DhtArc {
                center_loc: u32::MAX / 2,
                half_length: (u32::MAX / 2) as i32,
            },)
            .len()
    );
}
