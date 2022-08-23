#![allow(missing_docs)]
#![allow(dead_code)]

use holochain_types::prelude::ChainItem;

use super::*;

/// Specify the method to use when inserting records into the source chain
pub enum InsertRecordsMethod {
    /// Simply add the new records
    AppendOnly,
    /// Remove all existing records before adding the new ones
    Truncate,
    /// Find which existing records can remain which still constitute a valid
    /// chain after the new records are inserted, keeping those and discarding
    /// the rest.
    Graft,
}

pub(crate) async fn insert_records_into_source_chain(
    _handle: Arc<ConductorHandleImpl>,
    _cell_id: CellId,
    _truncate: bool,
    _validate: bool,
    _records: Vec<Record>,
) -> ConductorApiResult<()> {
    todo!("move function defined on handle into here")
}

/// Specifies a set of existing actions forming a chain, and a set of incoming actions
/// to attempt to "graft" onto the existing chain.
/// The existing actions are guaranteed to be ordered in descending sequence order,
/// and the incoming actions are guaranteed to be ordered in increasing sequence order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainGraft<A> {
    existing: Vec<A>,
    incoming: Vec<A>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Pivot {
    None,
    NewRoot,
    Index(usize),
}

impl<A: ChainItem> ChainGraft<A> {
    pub fn new(mut existing: Vec<A>, mut incoming: Vec<A>) -> Self {
        existing.sort_unstable_by_key(|r| u32::MAX - r.seq());
        incoming.sort_unstable_by_key(|r| r.seq());
        Self { existing, incoming }
    }

    /// Given a set of incoming records, find the maximal set of existing hashes
    /// which can be preserved, and the minimal set of incoming records to be committed,
    /// such that the new source chain will include all of the incoming records, all of
    /// the existing hashes returned, and none of the records which fall outside of
    /// either group.
    ///
    /// This has the effect of attempting to "graft" the incoming records onto the existing
    /// source chain (which may be a tree), and afterwards removing all other forks.
    /// If there is no place to graft the incoming records, then the incoming records list entirely
    /// specifies the new chain.
    ///
    /// If the first incoming record's previous hash matches the last existing hash,
    /// then we return both lists unchanged.
    ///
    /// If the first incoming record's previous hash matches none of the existing hashes,
    /// then return an emtpy existing list and the full incoming list.
    ///
    /// If the first incoming record's previous hash matches one of the existing hashes
    /// other than the existing top, then:
    /// - from the first existing hash to match, walk forwards, checking if existing
    ///   hashes match the incoming records. For each existing record which matches a incoming
    ///   record, keep that hash in the existing list and remove it from the incoming list,
    ///   so that it doesn't get committed twice.
    pub fn rebalance(self) -> Self {
        let (pivot, overlap) = self.pivot_and_overlap();
        if let Some(pivot) = pivot {
            Self {
                existing: self.existing[pivot - overlap..].to_vec(),
                incoming: self.incoming[overlap..].to_vec(),
            }
        } else {
            Self {
                existing: vec![],
                incoming: self.incoming,
            }
        }
    }

    fn pivot(&self) -> Pivot {
        if let Some(first) = self.incoming.first() {
            if first.prev_hash().is_none() {
                // If the first incoming item is a root item, return a special "pivot" beyond
                // the last existing item (the existing root). This works out mathematically.
                Pivot::NewRoot
            } else {
                self.existing
                    .iter()
                    .position(|e| {
                        Some(e.get_hash()) == first.prev_hash() && e.seq() + 1 == first.seq()
                    })
                    .map(Pivot::Index)
                    .unwrap_or(Pivot::None)
            }
        } else {
            Pivot::None
        }
    }

    fn pivot_and_overlap(&self) -> (Option<usize>, usize) {
        let take = match self.pivot() {
            Pivot::NewRoot => self.existing.len(),
            Pivot::Index(pivot) => pivot,
            Pivot::None => return (None, 0),
        };
        let overlap = self
            .existing
            .iter()
            .take(take)
            .rev()
            .zip(self.incoming.iter())
            .position(|(e, n)| e != n)
            .unwrap_or_else(|| take.min(self.incoming.len()));
        (Some(take), overlap)
    }
}

#[cfg(test)]
mod tests {
    use holochain_types::test_utils::chain::{self as tu, TestChainItem};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_pivot() {
        isotest::isotest!(TestChainItem => |iso| {
            let chain = |r| tu::chain(r).into_iter().map(|a| iso.create(a)).collect::<Vec<_>>();
            let forked_chain = |r| tu::forked_chain(r).into_iter().map(|a| iso.create(a)).collect::<Vec<_>>();
            let gap_chain = |r| tu::gap_chain(r).into_iter().map(|a| iso.create(a)).collect::<Vec<_>>();

            let case = ChainGraft::new(chain(0..3), chain(3..6));
            assert_eq!(case.pivot_and_overlap(), (Some(0), 0));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), chain(3..6)));

            let case = ChainGraft::new(chain(0..4), chain(3..6));
            assert_eq!(case.pivot_and_overlap(), (Some(1), 1));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..4), chain(4..6)));

            let case = ChainGraft::new(chain(0..3), chain(1..4));
            assert_eq!(case.pivot_and_overlap(), (Some(2), 2));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), chain(3..4)));

            let case = ChainGraft::new(chain(0..3), chain(0..4));
            assert_eq!(case.pivot_and_overlap(), (Some(3), 3));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), chain(3..4)));

            let case = ChainGraft::new(chain(0..5), chain(0..3));
            assert_eq!(case.pivot_and_overlap(), (Some(5), 3));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), vec![]));

            let case = ChainGraft::new(chain(0..2), chain(3..6));
            assert_eq!(case.pivot_and_overlap(), (None, 0));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(case.rebalance(), ChainGraft::new(vec![], chain(3..6)));

            let case = ChainGraft::new(chain(0..2), vec![]);
            assert_eq!(case.pivot_and_overlap(), (None, 0));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(case.rebalance(), ChainGraft::new(vec![], vec![]));

            let case = ChainGraft::new(vec![], chain(0..5));
            assert_eq!(case.pivot_and_overlap(), (Some(0), 0));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(case.rebalance(), ChainGraft::new(vec![], chain(0..5)));

            let case = ChainGraft::new(chain(0..3), forked_chain(&[0..0, 3..6]));
            assert_eq!(case.pivot_and_overlap(), (Some(0), 0));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(
                case.rebalance(),
                ChainGraft::new(chain(0..3), forked_chain(&[0..0, 3..6])),
            );

            let case = ChainGraft::new(chain(0..3), forked_chain(&[0..0, 4..6]));
            assert_eq!(case.pivot_and_overlap(), (None, 0));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(
                case.rebalance(),
                ChainGraft::new(vec![], forked_chain(&[0..0, 4..6])),
            );

            let case = ChainGraft::new(forked_chain(&[0..3, 3..6]), chain(2..6));
            assert_eq!(case.pivot_and_overlap(), (Some(4), 1));
            assert_eq!(
                case.clone().rebalance().incoming,
                case.clone().rebalance().rebalance().incoming
            );
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), chain(3..6)),);
        });
    }
}
