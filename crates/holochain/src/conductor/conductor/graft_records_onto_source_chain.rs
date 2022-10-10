use holochain_state::source_chain::SourceChain;
use holochain_types::prelude::ChainItem;

use super::*;

pub(crate) async fn graft_records_onto_source_chain(
    handle: ConductorHandle,
    cell_id: CellId,
    validate: bool,
    records: Vec<Record>,
) -> ConductorApiResult<()> {
    // Get or create the space for this cell.
    // Note: This doesn't require the cell be installed.
    let space = handle.get_or_create_space(cell_id.dna_hash())?;

    let chc = None;
    let network = handle
        .holochain_p2p()
        .to_dna(cell_id.dna_hash().clone(), chc);

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
            let (sah, entry) = el.into_inner();
            if sah.action().author() != cell_id.agent_pubkey() {
                return Err(StateMutationError::AuthorsMustMatch);
            }
            Ok((sah, ops, entry.into_option()))
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
                        holochain_sqlite::sql::sql_cell::DELETE_ACTIONS_AFTER_SEQ,
                        rusqlite::named_params! {
                            ":author": cell_id.agent_pubkey(),
                            ":seq": seq
                        },
                    )
                    .map_err(StateMutationError::from)?;
                }

                let mut ops_to_integrate = Vec::new();

                // Commit the records and ops to the authored db.
                for (sah, ops, entry) in data {
                    // Clippy is wrong :(
                    #[allow(clippy::needless_collect)]
                    let basis = ops
                        .iter()
                        .map(|op| op.dht_basis().clone())
                        .collect::<Vec<_>>();
                    ops_to_integrate.extend(
                        source_chain::put_raw(txn, sah, ops, entry)?
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
    handle: ConductorHandle,
    cell_id: &CellId,
    chain_top: &Option<(ActionHash, u32)>,
    records: &[Record],
) -> ConductorApiResult<()> {
    let space = handle.get_or_create_space(cell_id.dna_hash())?;
    let ribosome = handle.get_ribosome(cell_id.dna_hash())?;
    let chc = None;
    let network = handle
        .holochain_p2p()
        .to_dna(cell_id.dna_hash().clone(), chc);

    // Create a raw source chain to validate against because
    // genesis may not have been run yet.
    let workspace = SourceChainWorkspace::raw_empty(
        space.authored_db.clone(),
        space.dht_db.clone(),
        space.dht_query_cache.clone(),
        space.cache_db.clone(),
        handle.keystore().clone(),
        cell_id.agent_pubkey().clone(),
        Arc::new(ribosome.dna_def().as_content().clone()),
    )
    .await?;

    let sc = workspace.source_chain();

    // Validate the chain.
    crate::core::validate_chain(records.iter().map(|e| e.signed_action()), chain_top)
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
    .await?;

    Ok(())
}

/// Specifies a set of existing actions forming a chain, and a set of incoming actions
/// to attempt to "graft" onto the existing chain.
///
/// The existing actions are guaranteed to be ordered in descending sequence order,
/// and the incoming actions are guaranteed to be ordered in increasing sequence order.
/// This is just easier for implementation purposes.
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
    /// Constructor, ensuring that existing items are sorted descending,
    /// and incoming items are sorted ascending.
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
    /// - The existing actions form a chain, with no forks.
    /// - The incoming actions form a chain, with no forks.
    ///
    /// This has the effect of attempting to "graft" the incoming actions onto the existing
    /// source chain. If the grafting causes a fork, then the existing items after the fork
    /// point get deleted, so that there remains a single unforked chain containing the incoming items.
    ///
    /// If there is no place to graft the incoming actions, then the incoming actions list entirely
    /// specifies the new chain. i.e., if the first incoming record's previous hash matches none of
    /// the existing hashes, then return an empty existing list and the full incoming list.
    ///
    /// If the first incoming record's previous hash matches the last existing hash,
    /// then we return both lists unchanged.
    ///
    /// If the first incoming record's previous hash matches one of the existing hashes
    /// other than the existing top, then:
    /// - from the first existing hash to match, walk forwards, checking if existing
    ///   hashes match the incoming actions. For each existing record which matches an incoming
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
                // If the first incoming item is a root item, then there is no existing
                // item to use as the pivot, therefore we need to handle that case separately
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

    #[allow(dead_code)]
    pub fn existing(&self) -> &[A] {
        self.existing.as_ref()
    }

    pub fn incoming(&self) -> &[B] {
        self.incoming.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use holochain_types::test_utils::chain::{self as tu, TestChainItem};
    use test_case::test_case;

    use super::*;

    impl<A: ChainItem, B: Clone + AsRef<A>> ChainGraft<A, B> {
        pub fn map<T>(&self, f: impl Fn(A) -> T + Clone) -> ChainGraft<T, T> {
            ChainGraft {
                existing: self.existing.clone().into_iter().map(f.clone()).collect(),
                incoming: self
                    .incoming
                    .iter()
                    .map(AsRef::as_ref)
                    .cloned()
                    .map(f)
                    .collect(),
            }
        }
    }

    #[derive(PartialEq, Eq)]
    struct Answer {
        pivot: (Option<usize>, usize),
        rebalanced: ChainGraft<TestChainItem, TestChainItem>,
    }

    /// Rebalancing an already-balanced set of incoming records is a no-op
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::chain(3..6)), Answer {
        pivot: (Some(0), 0),
        rebalanced: ChainGraft::new(tu::chain(0..3), tu::chain(3..6)),
    } ; "1")]
    #[test_case(ChainGraft::new(tu::chain(0..4), tu::chain(3..6)), Answer {
        pivot: (Some(1), 1),
        rebalanced: ChainGraft::new(tu::chain(0..4), tu::chain(4..6)),
    } ; "2")]
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::chain(1..4)), Answer {
        pivot: (Some(2), 2),
        rebalanced: ChainGraft::new(tu::chain(0..3), tu::chain(3..4)),
    } ; "3")]
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::chain(0..4)), Answer {
        pivot: (Some(3), 3),
        rebalanced: ChainGraft::new(tu::chain(0..3), tu::chain(3..4)),
    } ; "4")]
    #[test_case(ChainGraft::new(tu::chain(0..5), tu::chain(0..3)), Answer {
        pivot: (Some(5), 3),
        rebalanced: ChainGraft::new(tu::chain(0..3), vec![]),
    } ; "5")]
    #[test_case(ChainGraft::new(tu::chain(0..2), tu::chain(3..6)), Answer {
        pivot: (None, 0),
        rebalanced: ChainGraft::new(vec![], tu::chain(3..6)),
    } ; "6")]
    #[test_case(ChainGraft::new(tu::chain(0..2), vec![]), Answer {
        pivot: (None, 0),
        rebalanced: ChainGraft::new(vec![], vec![]),
    } ; "7")]
    #[test_case(ChainGraft::new(vec![], tu::chain(0..5)), Answer {
        pivot: (Some(0), 0),
        rebalanced: ChainGraft::new(vec![], tu::chain(0..5)),
    } ; "8")]
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::forked_chain(&[0..0, 3..6])), Answer {
        pivot: (Some(0), 0),
        rebalanced: ChainGraft::new(tu::chain(0..3), tu::forked_chain(&[0..0, 3..6])),
    } ; "9")]
    #[test_case(ChainGraft::new(tu::chain(0..3), tu::forked_chain(&[0..0, 4..6])), Answer {
        pivot: (None, 0),
        rebalanced: ChainGraft::new(vec![], tu::forked_chain(&[0..0, 4..6])),
    } ; "10")]
    #[test_case(ChainGraft::new(tu::forked_chain(&[0..3, 3..6]), tu::chain(2..6)), Answer {
        pivot: (Some(4), 1),
        rebalanced: ChainGraft::new(tu::chain(0..3), tu::chain(3..6)),
    } ; "11")]
    #[test_case(ChainGraft::new(tu::gap_chain(&[0..3, 6..9]), tu::chain(3..6)), Answer {
        pivot: (Some(3), 0),
        rebalanced: ChainGraft::new(tu::chain(0..3), tu::chain(3..6)),
    } ; "12")]
    #[test_case(ChainGraft::new(tu::chain(0..6), tu::gap_chain(&[0..3, 6..9])), Answer {
        pivot: (Some(6), 3),
        rebalanced: ChainGraft::new(tu::chain(0..3), tu::chain(6..9)),
    } ; "13")]
    fn test_incoming_rebalance_idempotence(
        case: ChainGraft<TestChainItem, TestChainItem>,
        answer: Answer,
    ) {
        isotest::isotest!(TestChainItem => |iso_a| {
            let case = case.map(|a| iso_a.create(a));
            pretty_assertions::assert_eq!(case.pivot_and_overlap(), answer.pivot);
            pretty_assertions::assert_eq!(case.rebalance(), answer.rebalanced.map(|a| iso_a.create(a)));
        });

        pretty_assertions::assert_eq!(
            case.clone().rebalance().incoming,
            case.clone().rebalance().rebalance().incoming
        );
    }
}
