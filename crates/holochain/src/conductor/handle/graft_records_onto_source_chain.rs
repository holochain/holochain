#![allow(missing_docs)]
#![allow(dead_code)]

use holochain_state::source_chain::SourceChain;
use holochain_types::prelude::ChainItem;

use super::*;

/// Specify the method to use when inserting records into the source chain
pub enum InsertionMethod {
    /// Simply add the new records on top of the existing ones
    Append,
    /// Remove all existing records before adding the new ones
    Reset,
    /// Find which existing records can remain which still constitute a valid
    /// chain after the new records are inserted, keeping those and discarding
    /// the rest.
    Graft,
}

pub type ChainHead = Option<(ActionHash, u32)>;

pub(crate) async fn graft_records_onto_source_chain(
    handle: Arc<ConductorHandleImpl>,
    cell_id: CellId,
    validate: bool,
    records: Vec<Record>,
) -> ConductorApiResult<()> {
    // Get or create the space for this cell.
    // Note: This doesn't require the cell be installed.
    let space = handle.conductor.get_or_create_space(cell_id.dna_hash())?;

    let network = handle
        .conductor
        .holochain_p2p()
        .to_dna(cell_id.dna_hash().clone());

    let source_chain: SourceChain = space
        .source_chain(handle.keystore().clone(), cell_id.agent_pubkey().clone())
        .await?;

    let existing = source_chain
        .query(ChainQueryFilter::new().descending())
        .await?
        .into_iter()
        .map(|r| r.signed_action)
        .collect::<Vec<SignedActionHashed>>();

    let graft = ChainGraft::new(existing, records).rebalance();
    let chain_top = graft.existing_chain_top();

    if validate {
        validate_records(handle.clone(), &cell_id, &chain_top, graft.incoming()).await?;
    }

    // Produce the op lights for each record.
    let data = graft
        .incoming
        .into_iter()
        .map(|el| {
            let ops = produce_op_lights_from_records(vec![&el])?;
            // Check have the same author as cell.
            let (shh, entry) = el.into_inner();
            if shh.action().author() != cell_id.agent_pubkey() {
                return Err(StateMutationError::AuthorsMustMatch);
            }
            Ok((shh, ops, entry.into_option()))
        })
        .collect::<StateMutationResult<Vec<_>>>()?;

    // Commit the records to the source chain.
    let ops_to_integrate = space
        .authored_db
        .async_commit({
            let cell_id = cell_id.clone();
            move |txn| {
                if let Some((_, seq)) = chain_top {
                    // Remove records above the grafting position.
                    //
                    // NOTES:
                    // - the chain top may have moved since the grafting call began,
                    //   but it doesn't really matter, since we explicitly want to
                    //   clobber anything beyond the grafting point anyway.
                    // - if there is an existing fork, there may still be a fork after the
                    //   grafting. A more rigorous approach would thin out the existing
                    //   actions until a single fork is obtained.
                    txn.execute(
                        "DELETE FROM Action WHERE author = ? AND seq > ?",
                        rusqlite::params![cell_id.agent_pubkey(), seq],
                    )
                    .map_err(StateMutationError::from)?;
                }

                let mut ops_to_integrate = Vec::new();

                // Commit the records and ops to the authored db.
                for (shh, ops, entry) in data {
                    // Clippy is wrong :(
                    #[allow(clippy::needless_collect)]
                    let basis = ops
                        .iter()
                        .map(|op| op.dht_basis().clone())
                        .collect::<Vec<_>>();
                    ops_to_integrate.extend(
                        source_chain::put_raw(txn, shh, ops, entry)?
                            .into_iter()
                            .zip(basis.into_iter()),
                    );
                }
                SourceChainResult::Ok(ops_to_integrate)
            }
        })
        .await?;

    // Check which ops need to be integrated.
    // Only integrated if a cell is installed.
    if handle
        .list_cell_ids(Some(CellStatus::Joined))
        .contains(&cell_id)
    {
        holochain_state::integrate::authored_ops_to_dht_db(
            &network,
            ops_to_integrate,
            &space.authored_db,
            &space.dht_db,
            &space.dht_query_cache,
        )
        .await?;
    }
    Ok(())
}

async fn validate_records(
    handle: Arc<ConductorHandleImpl>,
    cell_id: &CellId,
    chain_top: &Option<(ActionHash, u32)>,
    records: &[Record],
) -> ConductorApiResult<()> {
    let space = handle.conductor.get_or_create_space(cell_id.dna_hash())?;
    let ribosome = handle.get_ribosome(cell_id.dna_hash())?;
    let network = handle
        .conductor
        .holochain_p2p()
        .to_dna(cell_id.dna_hash().clone());

    // Create a raw source chain to validate against because
    // genesis may not have been run yet.
    let workspace = SourceChainWorkspace::raw_empty(
        space.authored_db.clone(),
        space.dht_db.clone(),
        space.dht_query_cache.clone(),
        space.cache_db.clone(),
        handle.conductor.keystore().clone(),
        cell_id.agent_pubkey().clone(),
        Arc::new(ribosome.dna_def().as_content().clone()),
    )
    .await?;

    let sc = workspace.source_chain();

    // Validate the chain.
    crate::core::validate_chain(records.iter().map(|e| e.signed_action()), &chain_top)
        .map_err(|e| SourceChainError::InvalidCommit(e.to_string()))?;

    // Add the records to the source chain so we can validate them.
    sc.scratch()
        .apply(|scratch| {
            for r in records {
                holochain_state::prelude::insert_record_scratch(
                    scratch,
                    r.clone(),
                    Default::default(),
                );
            }
        })
        .map_err(SourceChainError::from)?;

    // Run the individual record validations.
    crate::core::workflow::inline_validation(
        workspace.clone(),
        network.clone(),
        handle.clone(),
        ribosome,
    )
    .await
    .map_err(Box::new)?;

    Ok(())
}

/// Specifies a set of existing actions forming a chain, and a set of incoming actions
/// to attempt to "graft" onto the existing chain.
/// The existing actions are guaranteed to be ordered in descending sequence order,
/// and the incoming actions are guaranteed to be ordered in increasing sequence order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainGraft<A, B> {
    existing: Vec<A>,
    incoming: Vec<B>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Pivot {
    None,
    NewRoot,
    Index(usize),
}

impl<A: ChainItem, B: Clone + AsRef<A>> ChainGraft<A, B> {
    pub fn new(mut existing: Vec<A>, mut incoming: Vec<B>) -> Self {
        existing.sort_unstable_by_key(|r| u32::MAX - r.seq());
        incoming.sort_unstable_by_key(|r| r.as_ref().seq());
        Self { existing, incoming }
    }

    pub fn existing_chain_top(&self) -> Option<(A::Hash, u32)> {
        self.existing
            .first()
            .map(|a| (a.get_hash().clone(), a.seq()))
    }

    /// Given a set of incoming actions, find the maximal set of existing hashes
    /// which can be preserved, and the minimal set of incoming actions to be committed,
    /// such that the new source chain will include all of the incoming actions, all of
    /// the existing hashes returned, and none of the actions which fall outside of
    /// either group.
    ///
    /// Assumptions:
    /// - The existing actions form a chain. The existence of a fork means UB.
    /// - The incoming actions form a chain. The existence of a fork means UB.
    ///
    /// This has the effect of attempting to "graft" the incoming actions onto the existing
    /// source chain (which may be a tree), and afterwards removing all other forks.
    /// If there is no place to graft the incoming actions, then the incoming actions list entirely
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
    ///   hashes match the incoming actions. For each existing record which matches a incoming
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
            if first.as_ref().prev_hash().is_none() {
                // If the first incoming item is a root item, return a special "pivot" beyond
                // the last existing item (the existing root). This works out mathematically.
                Pivot::NewRoot
            } else {
                self.existing
                    .iter()
                    .position(|e| {
                        Some(e.get_hash()) == first.as_ref().prev_hash()
                            && e.seq() + 1 == first.as_ref().seq()
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
            .position(|(e, n)| e != n.as_ref())
            .unwrap_or_else(|| take.min(self.incoming.len()));
        (Some(take), overlap)
    }

    pub fn existing(&self) -> &[A] {
        self.existing.as_ref()
    }

    pub fn incoming(&self) -> &[B] {
        self.incoming.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use holochain_types::test_utils::chain::{self as tu, TestChainHash, TestChainItem};
    use pretty_assertions::assert_eq;
    use test_case::test_case;

    use super::*;

    #[test]
    fn test_pivot_and_rebalance() {
        isotest::isotest!(TestChainItem, TestChainHash => |iso_a, iso_h| {
            let chain = |r| tu::chain(r).into_iter().map(|a| iso_a.create(a)).collect::<Vec<_>>();
            let forked_chain = |r| tu::forked_chain(r).into_iter().map(|a| iso_a.create(a)).collect::<Vec<_>>();
            let gap_chain = |r| tu::gap_chain(r).into_iter().map(|a| iso_a.create(a)).collect::<Vec<_>>();
            let empty = || Vec::<TestChainItem>::new().into_iter().map(|a| iso_a.create(a)).collect::<Vec<_>>();
            let top = |h| (iso_h.create(TestChainHash(h)), h);

            let case = ChainGraft::new(chain(0..3), chain(3..6));
            assert_eq!(case.pivot_and_overlap(), (Some(0), 0));
            assert_eq!(case.clone().rebalance(), ChainGraft::new(chain(0..3), chain(3..6)));
            assert_eq!(case.rebalance().existing_chain_top(), Some(top(2)));

            let case = ChainGraft::new(chain(0..4), chain(3..6));
            assert_eq!(case.pivot_and_overlap(), (Some(1), 1));
            assert_eq!(case.clone().rebalance(), ChainGraft::new(chain(0..4), chain(4..6)));
            assert_eq!(case.rebalance().existing_chain_top(), Some(top(3)));

            let case = ChainGraft::new(chain(0..3), chain(1..4));
            assert_eq!(case.pivot_and_overlap(), (Some(2), 2));
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), chain(3..4)));

            let case = ChainGraft::new(chain(0..3), chain(0..4));
            assert_eq!(case.pivot_and_overlap(), (Some(3), 3));
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), chain(3..4)));

            let case = ChainGraft::new(chain(0..5), chain(0..3));
            assert_eq!(case.pivot_and_overlap(), (Some(5), 3));
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), empty()));

            let case = ChainGraft::new(chain(0..2), chain(3..6));
            assert_eq!(case.pivot_and_overlap(), (None, 0));
            assert_eq!(case.rebalance(), ChainGraft::new(empty(), chain(3..6)));

            let case = ChainGraft::new(chain(0..2), empty());
            assert_eq!(case.pivot_and_overlap(), (None, 0));
            assert_eq!(case.rebalance(), ChainGraft::new(empty(), empty()));

            let case = ChainGraft::new(empty(), chain(0..5));
            assert_eq!(case.pivot_and_overlap(), (Some(0), 0));
            assert_eq!(case.rebalance(), ChainGraft::new(empty(), chain(0..5)));

            let case = ChainGraft::new(chain(0..3), forked_chain(&[0..0, 3..6]));
            assert_eq!(case.pivot_and_overlap(), (Some(0), 0));
            assert_eq!(
                case.rebalance(),
                ChainGraft::new(chain(0..3), forked_chain(&[0..0, 3..6])),
            );

            let case = ChainGraft::new(chain(0..3), forked_chain(&[0..0, 4..6]));
            assert_eq!(case.pivot_and_overlap(), (None, 0));
            assert_eq!(
                case.rebalance(),
                ChainGraft::new(empty(), forked_chain(&[0..0, 4..6])),
            );

            let case = ChainGraft::new(forked_chain(&[0..3, 3..6]), chain(2..6));
            assert_eq!(case.pivot_and_overlap(), (Some(4), 1));
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), chain(3..6)),);

            let case = ChainGraft::new(gap_chain(&[0..3, 6..9]), chain(3..6));
            assert_eq!(case.pivot_and_overlap(), (Some(3), 0));
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), chain(3..6)),);

            let case = ChainGraft::new(chain(0..6), gap_chain(&[0..3, 6..9]));
            assert_eq!(case.pivot_and_overlap(), (Some(6), 3));
            assert_eq!(case.rebalance(), ChainGraft::new(chain(0..3), chain(6..9)),);
        });
    }

    /// Rebalancing an already-balanced set of incoming records is a no-op
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::chain(3..6)))]
    #[test_case(ChainGraft::new(tu::chain(0..4), tu::chain(3..6)))]
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::chain(1..4)))]
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::chain(0..4)))]
    #[test_case(ChainGraft::new(tu::chain(0..5), tu::chain(0..3)))]
    #[test_case(ChainGraft::new(tu::chain(0..2), tu::chain(3..6)))]
    #[test_case(ChainGraft::new(tu::chain(0..2), vec![]))]
    #[test_case(ChainGraft::new(vec![], tu::chain(0..5)))]
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::forked_chain(&[0..0, 3..6])))]
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::forked_chain(&[0..0, 4..6])))]
    #[test_case(ChainGraft::new(tu::forked_chain(&[0..3, 3..6]), tu::chain(2..6)))]
    #[test_case(ChainGraft::new(tu::gap_chain(&[0..3, 6..9]), tu::chain(3..6)))]
    #[test_case(ChainGraft::new(tu::chain(0..6), tu::gap_chain(&[0..3, 6..9])))]
    fn test_incoming_rebalance_idempotence(case: ChainGraft<TestChainItem, TestChainItem>) {
        pretty_assertions::assert_eq!(
            case.clone().rebalance().incoming,
            case.clone().rebalance().rebalance().incoming
        );
    }
}
