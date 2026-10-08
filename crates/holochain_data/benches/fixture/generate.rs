//! Builds signed records for a set of authors without touching a database.

use super::FixtureConfig;
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash};
use holochain_integrity_types::action::{
    Action, ActionData, ActionHeader, AgentValidationPkgData, AppEntryDef, CreateData,
    CreateLinkData, DeleteData, DeleteLinkData, DnaData, EntryDefIndex, EntryType,
    InitZomesCompleteData, UpdateData, ZomeIndex,
};
use holochain_integrity_types::entry::{AppEntryBytes, Entry};
use holochain_integrity_types::entry_def::EntryVisibility;
use holochain_integrity_types::link::{LinkTag, LinkType};
use holochain_integrity_types::record::{Record, RecordEntry, SignedHashed};
use holochain_integrity_types::signature::Signature;
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use holochain_timestamp::Timestamp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// One author's chain.
pub struct GeneratedChain {
    pub author: AgentPubKey,
    pub records: Vec<Record>,
}

/// Everything the writer needs.
pub struct Generated {
    pub local_author: AgentPubKey,
    pub chains: Vec<GeneratedChain>,
    pub link_bases: Vec<AnyLinkableHash>,
    pub min_timestamp: i64,
    pub max_timestamp: i64,
}

/// Number of link bases in the pool; the first `HOT_BASES` get most links.
const BASE_POOL: usize = 512;
const HOT_BASES: usize = 8;
/// Base timestamp: 2026-01-01T00:00:00Z in micros.
const T0: i64 = 1_767_225_600_000_000;

fn authors_for(actions: usize) -> usize {
    (actions / 250).clamp(4, 400)
}

fn random_bytes(rng: &mut StdRng, n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    rng.fill(&mut v[..]);
    v
}

fn random_agent(rng: &mut StdRng) -> AgentPubKey {
    AgentPubKey::from_raw_36(random_bytes(rng, 36))
}

fn random_signature(rng: &mut StdRng) -> Signature {
    let mut s = [0u8; 64];
    rng.fill(&mut s[..]);
    Signature(s)
}

/// Entry size log-uniform between 32 B and 8 KiB.
fn random_entry(rng: &mut StdRng) -> Entry {
    let exp: f64 = rng.random_range(5.0..13.0);
    let len = 2f64.powf(exp) as usize;
    let bytes = SerializedBytes::from(UnsafeBytes::from(random_bytes(rng, len)));
    Entry::App(AppEntryBytes::try_from(bytes).expect("entry below size limit"))
}

fn pick_base(rng: &mut StdRng, pool: &[AnyLinkableHash]) -> AnyLinkableHash {
    // 70% of links go to the hot bases, the rest spread over the pool.
    if rng.random_bool(0.7) {
        pool[rng.random_range(0..HOT_BASES)].clone()
    } else {
        pool[rng.random_range(0..pool.len())].clone()
    }
}

struct ChainBuilder<'a> {
    rng: &'a mut StdRng,
    author: AgentPubKey,
    is_local: bool,
    seq: u32,
    prev: Option<ActionHash>,
    ts: i64,
    creates: Vec<(ActionHash, EntryHash)>,
    links: Vec<(ActionHash, AnyLinkableHash)>,
    records: Vec<Record>,
}

impl ChainBuilder<'_> {
    fn push(&mut self, data: ActionData, entry: Option<Entry>, vis: Option<EntryVisibility>) {
        let action = Action {
            header: ActionHeader {
                author: self.author.clone(),
                timestamp: Timestamp::from_micros(self.ts),
                action_seq: self.seq,
                prev_action: self.prev.clone(),
            },
            data,
        };
        let signed = SignedHashed::new_unchecked(action, random_signature(self.rng));
        self.prev = Some(signed.as_hash().clone());
        self.seq += 1;
        self.ts += self.rng.random_range(1_000..5_000_000);
        self.records
            .push(Record::new(signed, RecordEntry::new(vis.as_ref(), entry)));
    }

    fn genesis(&mut self, dna_hash: &DnaHash) {
        self.push(
            ActionData::Dna(DnaData {
                dna_hash: dna_hash.clone(),
            }),
            None,
            None,
        );
        self.push(
            ActionData::AgentValidationPkg(AgentValidationPkgData {
                membrane_proof: None,
            }),
            None,
            None,
        );
        let agent_entry = Entry::Agent(self.author.clone());
        let entry_hash = EntryHash::with_data_sync(&agent_entry);
        let hash_for_list = entry_hash.clone();
        self.push(
            ActionData::Create(CreateData {
                entry_type: EntryType::AgentPubKey,
                entry_hash,
            }),
            Some(agent_entry),
            Some(EntryVisibility::Public),
        );
        let create_hash = self.prev.clone().expect("just pushed");
        self.creates.push((create_hash, hash_for_list));
        self.push(ActionData::InitZomesComplete(InitZomesCompleteData {}), None, None);
    }

    fn create(&mut self) {
        let private = self.rng.random_bool(0.1);
        let vis = if private {
            EntryVisibility::Private
        } else {
            EntryVisibility::Public
        };
        let entry = random_entry(self.rng);
        let entry_hash = EntryHash::with_data_sync(&entry);
        let entry_type = EntryType::App(AppEntryDef::new(
            EntryDefIndex(self.rng.random_range(0..4)),
            ZomeIndex(0),
            vis,
        ));
        // Authorities never hold other agents' private entries.
        let stored_entry = if private && !self.is_local {
            None
        } else {
            Some(entry)
        };
        self.push(
            ActionData::Create(CreateData {
                entry_type,
                entry_hash: entry_hash.clone(),
            }),
            stored_entry,
            Some(vis),
        );
        let hash = self.prev.clone().expect("just pushed");
        if !private {
            self.creates.push((hash, entry_hash));
        }
    }

    fn update(&mut self) {
        let Some(idx) = self.pick_create() else {
            return self.create();
        };
        let (orig_action, orig_entry) = self.creates[idx].clone();
        let entry = random_entry(self.rng);
        let entry_hash = EntryHash::with_data_sync(&entry);
        self.push(
            ActionData::Update(UpdateData {
                original_action_address: orig_action,
                original_entry_address: orig_entry,
                entry_type: EntryType::App(AppEntryDef::new(
                    EntryDefIndex(0),
                    ZomeIndex(0),
                    EntryVisibility::Public,
                )),
                entry_hash: entry_hash.clone(),
            }),
            Some(entry),
            Some(EntryVisibility::Public),
        );
        let hash = self.prev.clone().expect("just pushed");
        self.creates.push((hash, entry_hash));
    }

    fn delete(&mut self) {
        let Some(idx) = self.pick_create() else {
            return self.create();
        };
        let (action, entry) = self.creates.swap_remove(idx);
        self.push(
            ActionData::Delete(DeleteData {
                deletes_address: action,
                deletes_entry_address: entry,
            }),
            None,
            None,
        );
    }

    fn create_link(&mut self, pool: &[AnyLinkableHash]) {
        let base = pick_base(self.rng, pool);
        let target = pick_base(self.rng, pool);
        let tag_len = self.rng.random_range(0..32);
        let tag = random_bytes(self.rng, tag_len);
        let link_type = LinkType(self.rng.random_range(0..3));
        self.push(
            ActionData::CreateLink(CreateLinkData {
                base_address: base.clone(),
                target_address: target,
                zome_index: ZomeIndex(0),
                link_type,
                tag: LinkTag::new(tag),
            }),
            None,
            None,
        );
        let hash = self.prev.clone().expect("just pushed");
        self.links.push((hash, base));
    }

    fn delete_link(&mut self, pool: &[AnyLinkableHash]) {
        if self.links.is_empty() {
            return self.create_link(pool);
        }
        let idx = self.rng.random_range(0..self.links.len());
        let (create, base) = self.links.swap_remove(idx);
        self.push(
            ActionData::DeleteLink(DeleteLinkData {
                base_address: base,
                link_add_address: create,
            }),
            None,
            None,
        );
    }

    fn pick_create(&mut self) -> Option<usize> {
        // Skip the genesis agent entry at index 0 so it is never deleted.
        if self.creates.len() < 2 {
            return None;
        }
        Some(self.rng.random_range(1..self.creates.len()))
    }
}

/// Build the chains for `cfg`.
pub fn generate(cfg: FixtureConfig) -> Generated {
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let dna_hash = DnaHash::from_raw_36(random_bytes(&mut rng, 36));
    let n_authors = authors_for(cfg.actions);
    let per_author = cfg.actions / n_authors;
    let authors: Vec<AgentPubKey> = (0..n_authors).map(|_| random_agent(&mut rng)).collect();
    let pool: Vec<AnyLinkableHash> = (0..BASE_POOL)
        .map(|_| EntryHash::from_raw_36(random_bytes(&mut rng, 36)).into())
        .collect();

    let mut chains = Vec::with_capacity(n_authors);
    let mut min_ts = i64::MAX;
    let mut max_ts = i64::MIN;
    for (i, author) in authors.iter().enumerate() {
        let mut b = ChainBuilder {
            ts: T0 + rng.random_range(0..86_400_000_000),
            rng: &mut rng,
            author: author.clone(),
            is_local: i == 0,
            seq: 0,
            prev: None,
            creates: Vec::new(),
            links: Vec::new(),
            records: Vec::with_capacity(per_author),
        };
        b.genesis(&dna_hash);
        while b.records.len() < per_author {
            match b.rng.random_range(0..100u32) {
                0..40 => b.create(),
                40..55 => b.update(),
                55..65 => b.delete(),
                65..90 => b.create_link(&pool),
                _ => b.delete_link(&pool),
            }
        }
        let first = b.records.first().expect("genesis").action().timestamp().as_micros();
        let last = b.records.last().expect("genesis").action().timestamp().as_micros();
        min_ts = min_ts.min(first);
        max_ts = max_ts.max(last);
        chains.push(GeneratedChain {
            author: author.clone(),
            records: b.records,
        });
    }

    Generated {
        local_author: authors[0].clone(),
        chains,
        link_bases: pool[..HOT_BASES].to_vec(),
        min_timestamp: min_ts,
        max_timestamp: max_ts,
    }
}
