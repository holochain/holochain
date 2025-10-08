use crate::conductor::integration_dump;
use crate::sweettest::DurationOrSeconds;
use crate::sweettest::SweetCell;
use crate::test_utils::consistency::request_published_ops;
use holo_hash::*;
use holochain_conductor_api::IntegrationStateDump;
use holochain_conductor_api::IntegrationStateDumps;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::prelude::StateQueryResult;
use holochain_types::prelude::*;
use rusqlite::named_params;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write;
use std::time::Duration;

/// Consistency was failed to be reached. Here's a report.
#[derive(derive_more::From)]
pub struct ConsistencyError(String);

impl std::fmt::Debug for ConsistencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Alias
pub type ConsistencyResult = Result<(), ConsistencyError>;

async fn delay(elapsed: Duration) {
    let delay = if elapsed > Duration::from_secs(10) {
        CONSISTENCY_DELAY_HIGH
    } else if elapsed > Duration::from_secs(1) {
        CONSISTENCY_DELAY_MID
    } else {
        CONSISTENCY_DELAY_LOW
    };
    tokio::time::sleep(delay).await
}

/// Extra conditions that must be satisfied for consistency to be reached.
///
/// Without supplying extra conditions, it's expected that at the time of beginning
/// the consistency awaiting, all ops which will be published have already been published.
/// However, in cases where more publishing is expected, such as when warrants will be authored
/// due to recently publishing invalid ops, these conditions can be used to make sure that
/// the consistency check will not proceed until all publishing expectations have occurred.
#[derive(Debug, Default, Clone)]
pub struct ConsistencyConditions {
    /// This many warrants must have been published against the keyed agent.
    warrants_issued: HashMap<AgentPubKey, usize>,
}

impl From<()> for ConsistencyConditions {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl From<Vec<(AgentPubKey, usize)>> for ConsistencyConditions {
    fn from(items: Vec<(AgentPubKey, usize)>) -> Self {
        Self {
            warrants_issued: items.iter().cloned().collect(),
        }
    }
}

impl ConsistencyConditions {
    fn check<'a>(
        &self,
        published_ops: impl Iterator<Item = &'a DhtOp>,
    ) -> Result<bool, ConsistencyError> {
        let mut checked = self.warrants_issued.clone();
        for v in checked.values_mut() {
            *v = 0;
        }

        for op in published_ops {
            if let DhtOp::WarrantOp(op) = op {
                let author = &op.action_author();
                if let Some(count) = checked.get_mut(author) {
                    *count += 1;
                    if *count > *self.warrants_issued.get(author).unwrap() {
                        return Err(format!(
                            "Expected exactly {} warrants to be published against agent {author}, but found more",
                            self.warrants_issued.get(author).unwrap(),
                        )
                    .into());
                    }
                }
            }
        }

        Ok(checked == self.warrants_issued)
    }

    /// Return the total number of warrants expected to be published
    pub fn num_warrants(&self) -> usize {
        self.warrants_issued.values().sum()
    }
}

/// Wait for all cells to reach consistency,
/// with the option to specify that some cells are offline.
///
/// Cells paired with a `false` value will have their authored ops counted towards the total,
/// but not their integrated ops (since they are not online to integrate things).
/// This is useful for tests where nodes go offline.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn await_conditional_consistency<'a, I: IntoIterator<Item = (&'a SweetCell, bool)>>(
    timeout: impl Into<DurationOrSeconds>,
    conditions: impl Into<ConsistencyConditions>,
    all_cells: I,
) -> ConsistencyResult {
    #[allow(clippy::type_complexity)]
    let all_cell_dbs: Vec<(
        AgentPubKey,
        DbRead<DbKindAuthored>,
        Option<DbRead<DbKindDht>>,
    )> = all_cells
        .into_iter()
        .map(|(c, online)| {
            (
                c.agent_pubkey().clone(),
                c.authored_db().clone().into(),
                online.then(|| c.dht_db().clone().into()),
            )
        })
        .collect();
    let all_cell_dbs: Vec<_> = all_cell_dbs
        .iter()
        .map(|c| (&c.0, &c.1, c.2.as_ref()))
        .collect();
    wait_for_integration_diff_conditional(
        &all_cell_dbs[..],
        timeout.into().into_duration(),
        conditions.into(),
    )
    .await
}

/// Wait for all cell envs to reach consistency, meaning that every op
/// published by every cell has been integrated by every node
pub async fn wait_for_integration_diff_conditional<AuthorDb, DhtDb>(
    cells: &[(&AgentPubKey, &AuthorDb, Option<&DhtDb>)],
    timeout: Duration,
    conditions: ConsistencyConditions,
) -> ConsistencyResult
where
    AuthorDb: ReadAccess<DbKindAuthored>,
    DhtDb: ReadAccess<DbKindDht>,
{
    let start = tokio::time::Instant::now();
    let mut done = HashSet::new();
    let mut integrated = vec![HashSet::new(); cells.len()];
    let mut published = HashSet::new();
    let mut publish_complete = false;

    while start.elapsed() < timeout {
        if !publish_complete {
            published = HashSet::new();
            for (_author, db, _) in cells.iter() {
                // Providing the author is redundant
                let p = request_published_ops(*db, None /*Some((*author).to_owned())*/)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|(_, _, op)| op);

                // Assert that there are no duplicates
                let expected = p.len() + published.len();
                published.extend(p);
                assert_eq!(published.len(), expected);
            }
        }

        let prev_publish_complete = publish_complete;
        publish_complete = conditions.check(published.iter())?;

        if publish_complete {
            if !prev_publish_complete {
                tracing::info!("*** All expected ops were published ***");
            }
            // Compare the published ops to the integrated ops for each node
            for (i, (_, _, dht_db)) in cells.iter().enumerate() {
                if done.contains(&i) {
                    continue;
                }
                if let Some(db) = dht_db.as_ref() {
                    integrated[i] = get_integrated_ops(*db).await.into_iter().collect();

                    if integrated[i] == published {
                        done.insert(i);
                        tracing::debug!(i, "Node reached consistency");
                    } else {
                        let total_time_waited = start.elapsed();
                        let queries = query_integration(*db).await;
                        let num_integrated = integrated.len();
                        tracing::debug!(i, ?num_integrated, ?total_time_waited, counts = ?queries, "consistency-status");
                    }
                } else {
                    // If the DHT db is not provided, don't check integration
                    done.insert(i);
                }
            }
        }

        // If all nodes reached consistency, exit successfully
        if done.len() == cells.len() {
            return Ok(());
        }

        let total_time_waited = start.elapsed();
        delay(total_time_waited).await;
    }

    let header = format!(
        "{:53} {:>3} {:53} {:53} {}\n{}",
        "author",
        "seq",
        "op_hash",
        "action_hash",
        "op_type (action_type)",
        "-".repeat(53 + 3 + 53 + 53 + 4 + 21)
    );

    if !publish_complete {
        let published = published
            .iter()
            .map(display_op)
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("There are still ops which were expected to have been published which weren't:\n{header}\n{published}").into());
    }

    let mut report = String::new();
    let not_consistent = (0..cells.len())
        .filter(|i| !done.contains(i))
        .collect::<Vec<_>>();

    writeln!(
        report,
        "{} cells did not reach consistency: {:?}",
        not_consistent.len(),
        not_consistent
    )
    .unwrap();

    if not_consistent.is_empty() {
        unreachable!("At least one node must not have reached consistency");
    }

    for c in &not_consistent {
        let integrated = integrated[*c].clone();

        eprintln!("Agent {} is not consistent", cells[*c].0);

        let (unintegrated, unpublished) = diff_ops(published.iter(), integrated.iter());
        let diff = diff_report(unintegrated, unpublished);

        #[allow(clippy::comparison_chain)]
        if integrated.len() > published.len() {
            eprintln!(
                "{report}\nnum integrated ops ({}) > num published ops ({}), meaning you may not be accounting for all nodes in this test. Consistency may not be complete. Report:\n\n{header}\n{diff}",
                integrated.len(),
                published.len()
            );
        } else if integrated.len() < published.len() {
            let db = cells[*c].2.as_ref().expect("DhtDb must be provided");
            let integration_dump = integration_dump(*db).await.unwrap();

            eprintln!(
                "{}\nConsistency not achieved after {:?}. Expected {} ops, but only {} integrated. Report:\n\n{}\n{}\n\n{:?}",
                report,
                timeout,
                published.len(),
                integrated.len(),
                header,
                diff,
                integration_dump
            );
        } else {
            unreachable!()
        }
    }

    Err(ConsistencyError(format!(
        "{} agents were inconsistent",
        not_consistent.len()
    )))
}

const CONSISTENCY_DELAY_LOW: Duration = Duration::from_millis(100);
const CONSISTENCY_DELAY_MID: Duration = Duration::from_millis(500);
const CONSISTENCY_DELAY_HIGH: Duration = Duration::from_millis(1000);

fn diff_ops<'a>(
    published: impl Iterator<Item = &'a DhtOp>,
    integrated: impl Iterator<Item = &'a DhtOp>,
) -> (Vec<String>, Vec<String>) {
    let mut published: Vec<_> = published.map(display_op).collect();
    let mut integrated: Vec<_> = integrated.map(display_op).collect();
    published.sort();
    integrated.sort();

    let mut unintegrated = vec![];
    let mut unpublished = vec![];

    for d in diff::slice(&published, &integrated) {
        match d {
            diff::Result::Left(l) => unintegrated.push(l.to_owned()),
            diff::Result::Right(r) => unpublished.push(r.to_owned()),
            _ => (),
        }
    }

    (unintegrated, unpublished)
}

fn diff_report(unintegrated: Vec<String>, unpublished: Vec<String>) -> String {
    let unintegrated = if unintegrated.is_empty() {
        "".to_string()
    } else {
        format!("Unintegrated:\n\n{}\n", unintegrated.join("\n"))
    };
    let unpublished = if unpublished.is_empty() {
        "".to_string()
    } else {
        format!("Unpublished:\n\n{}\n", unpublished.join("\n"))
    };

    format!("{unintegrated}{unpublished}")
}

fn display_op(op: &DhtOp) -> String {
    match op {
        DhtOp::ChainOp(op) => format!(
            "{} {:>3} {} {} {} ({})",
            op.action().author(),
            op.action().action_seq(),
            op.to_hash(),
            op.action().to_hash(),
            op.get_type(),
            op.action().action_type(),
        ),
        DhtOp::WarrantOp(op) => {
            format!("{} WARRANT ({})", op.author, op.get_type(),)
        }
    }
}

/// Wait for num_attempts * delay, or until all published ops have been integrated.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(db)))]
pub async fn wait_for_integration<Db: ReadAccess<DbKindDht>>(
    db: &Db,
    num_published: usize,
    num_attempts: usize,
    delay: Duration,
) -> Result<(), String> {
    let mut num_integrated = 0;
    for i in 0..num_attempts {
        num_integrated = get_integrated_count(db).await;
        if num_integrated >= num_published {
            if num_integrated > num_published {
                tracing::warn!("num integrated ops > num published ops, meaning you may not be accounting for all nodes in this test.
                Consistency may not be complete.")
            }
            return Ok(());
        } else {
            let total_time_waited = delay * i as u32;
            tracing::debug!(?num_integrated, ?total_time_waited, counts = ?query_integration(db).await, "consistency-status");
        }
        tokio::time::sleep(delay).await;
    }

    Err(format!(
        "Consistency not achieved after {num_attempts} attempts. Expected {num_published} ops, but only {num_integrated} integrated.",
    ))
}

/// Show authored data for each cell environment
#[cfg_attr(feature = "instrument", tracing::instrument(skip(envs)))]
pub async fn show_authored<Db: ReadAccess<DbKindAuthored>>(envs: &[&Db]) {
    for (i, &db) in envs.iter().enumerate() {
        db.read_async(move |txn| -> DatabaseResult<()> {
            txn.prepare("SELECT DISTINCT Action.seq, Action.type, Action.entry_hash FROM Action JOIN DhtOp ON Action.hash = DhtOp.hash")
            .unwrap()
            .query_map([], |row| {
                let action_type: String = row.get("type")?;
                let seq: u32 = row.get("seq")?;
                let entry: Option<EntryHash> = row.get("entry_hash")?;
                Ok((action_type, seq, entry))
            })
            .unwrap()
            .for_each(|r|{
                let (action_type, seq, entry) = r.unwrap();
                tracing::debug!(chain = %i, %seq, ?action_type, ?entry);
            });

            Ok(())
        }).await.unwrap();
    }
}

/// Get multiple db states with compact Display representation
pub async fn get_integration_dumps<Db: ReadAccess<DbKindDht>>(
    dbs: &[&Db],
) -> IntegrationStateDumps {
    let mut output = Vec::new();
    for db in dbs {
        let db = *db;
        output.push(query_integration(db).await);
    }
    IntegrationStateDumps(output)
}

/// Show the current db state.
pub async fn query_integration<Db: ReadAccess<DbKindDht>>(db: &Db) -> IntegrationStateDump {
    crate::conductor::integration_dump(&db.clone().into())
        .await
        .unwrap()
}

async fn get_integrated_count<Db: ReadAccess<DbKindDht>>(db: &Db) -> usize {
    db.read_async(move |txn| -> DatabaseResult<usize> {
        Ok(txn.query_row(
            "SELECT COUNT(hash) FROM DhtOp WHERE DhtOp.when_integrated IS NOT NULL",
            [],
            |row| row.get(0),
        )?)
    })
    .await
    .unwrap()
}

/// Get count of ops that have been successfully validated but not integrated
pub async fn get_valid_and_not_integrated_count<Db: ReadAccess<DbKindDht>>(db: &Db) -> usize {
    db.read_async(move |txn| -> DatabaseResult<usize> {
        Ok(txn.query_row(
            "SELECT COUNT(hash) FROM DhtOp WHERE when_integrated IS NULL AND validation_status = :status",
            named_params!{
                ":status": ValidationStatus::Valid,
            },
            |row| row.get(0),
        )?)
    })
    .await
    .unwrap()
}

/// Get count of ops that have been successfully validated and integrated
pub async fn get_valid_and_integrated_count<Db: ReadAccess<DbKindDht>>(db: &Db) -> usize {
    db.read_async(move |txn| -> DatabaseResult<usize> {
        Ok(txn.query_row(
            "SELECT COUNT(hash) FROM DhtOp WHERE when_integrated IS NOT NULL AND validation_status = :status",
            named_params!{
                ":status": ValidationStatus::Valid,
            },
            |row| row.get(0),
        )?)
    })
    .await
    .unwrap()
}

/// Get all [`DhtOps`](holochain_types::prelude::DhtOp) integrated by this node
pub async fn get_integrated_ops<Db: ReadAccess<DbKindDht>>(db: &Db) -> Vec<DhtOp> {
    db.read_async(move |txn| -> StateQueryResult<Vec<DhtOp>> {
        txn.prepare(
            "
            SELECT
            DhtOp.type, 
            Action.author as author, 
            Action.blob as action_blob, 
            Entry.blob as entry_blob
            FROM DhtOp
            JOIN
            Action ON DhtOp.action_hash = Action.hash
            LEFT JOIN
            Entry ON Action.entry_hash = Entry.hash
            WHERE
            DhtOp.when_integrated IS NOT NULL
            ORDER BY DhtOp.rowid ASC
        ",
        )
        .unwrap()
        .query_and_then(named_params! {}, |row| {
            Ok(holochain_state::query::map_sql_dht_op(true, "type", row).unwrap())
        })
        .unwrap()
        .collect::<StateQueryResult<_>>()
    })
    .await
    .unwrap()
}
