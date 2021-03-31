use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::Row;
use holochain_sqlite::rusqlite::Statement;
use holochain_sqlite::rusqlite::{Transaction, NO_PARAMS};
use holochain_sqlite::scratch::Scratch;
use holochain_sqlite::{impl_to_sql_via_display, rusqlite::TransactionBehavior};
use holochain_sqlite::{
    rusqlite::{named_params, Connection},
    schema::SCHEMA_CELL,
};
use holochain_types::dht_op::DhtOpLight;
use holochain_types::dht_op::{DhtOpHashed, DhtOpType};
use holochain_types::EntryHashed;
use holochain_types::{dht_op::DhtOp, header::NewEntryHeader};
use holochain_zome_types::*;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;

struct LinkTestData {
    create_link_op: DhtOpHashed,
    delete_link_op: DhtOpHashed,
    link: Link,
    base_op: DhtOpHashed,
    target_op: DhtOpHashed,
    base_query: LinkQuery,
    tag_query: LinkQuery,
}

struct EntryTestData {
    store_entry_op: DhtOpHashed,
    delete_entry_header_op: DhtOpHashed,
    entry: Entry,
    query: GetQuery,
    header: SignedHeaderHashed,
}

impl LinkTestData {
    fn new() -> Self {
        let mut create_link = fixt!(CreateLink);
        let mut delete_link = fixt!(DeleteLink);

        let mut create_base = fixt!(Create);
        let base = fixt!(Entry);
        let base_hash = EntryHash::with_data_sync(&base);
        create_base.entry_hash = base_hash.clone();

        let mut create_target = fixt!(Create);
        let target = fixt!(Entry);
        let target_hash = EntryHash::with_data_sync(&target);
        create_target.entry_hash = target_hash.clone();

        create_link.base_address = base_hash.clone();
        create_link.target_address = target_hash.clone();

        let create_link_sig = fixt!(Signature);
        let create_link_op = DhtOp::RegisterAddLink(create_link_sig.clone(), create_link.clone());

        let create_link_hash = HeaderHash::with_data_sync(&Header::CreateLink(create_link.clone()));

        delete_link.link_add_address = create_link_hash.clone();
        delete_link.base_address = base_hash.clone();

        let delete_link_op = DhtOp::RegisterRemoveLink(fixt!(Signature), delete_link.clone());

        let base_op = DhtOp::StoreEntry(
            fixt!(Signature),
            NewEntryHeader::Create(create_base.clone()),
            Box::new(base.clone()),
        );

        let target_op = DhtOp::StoreEntry(
            fixt!(Signature),
            NewEntryHeader::Create(create_target.clone()),
            Box::new(target.clone()),
        );

        let link = Link {
            target: target_hash.clone(),
            timestamp: create_link.timestamp.clone(),
            tag: create_link.tag.clone(),
            create_link_hash: create_link_hash.clone(),
        };

        let base_query = LinkQuery::base(base_hash.clone(), create_link.zome_id.clone());
        let tag_query = LinkQuery::tag(
            base_hash.clone(),
            create_link.zome_id.clone(),
            create_link.tag.clone(),
        );

        Self {
            create_link_op: DhtOpHashed::from_content_sync(create_link_op),
            delete_link_op: DhtOpHashed::from_content_sync(delete_link_op),
            link,
            base_op: DhtOpHashed::from_content_sync(base_op),
            target_op: DhtOpHashed::from_content_sync(target_op),
            base_query,
            tag_query,
        }
    }
}

impl EntryTestData {
    fn new() -> Self {
        let mut create = fixt!(Create);
        let mut delete = fixt!(Delete);
        let entry = fixt!(Entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        create.entry_hash = entry_hash.clone();

        let create_hash = HeaderHash::with_data_sync(&Header::Create(create.clone()));

        delete.deletes_entry_address = entry_hash.clone();
        delete.deletes_address = create_hash.clone();

        let signature = fixt!(Signature);
        let store_entry_op = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryHeader::Create(create.clone()),
            Box::new(entry.clone()),
        ));

        let header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::Create(create.clone())),
            signature.clone(),
        );

        let signature = fixt!(Signature);
        let delete_entry_header_op = DhtOpHashed::from_content_sync(
            DhtOp::RegisterDeletedEntryHeader(signature.clone(), delete.clone()),
        );

        let query = GetQuery(entry_hash.clone());

        Self {
            store_entry_op,
            header,
            entry,
            query,
            delete_entry_header_op,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_links() {
    observability::test_run().ok();
    let mut scratch = Scratch::default();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut cache = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut cache, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let mut cache_txn = cache
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = LinkTestData::new();

    // - Add link to db.
    insert_op(&mut txn, td.base_op.clone());
    insert_op(&mut txn, td.target_op.clone());
    insert_op(&mut txn, td.create_link_op.clone());

    // - Check we can get the link query back.
    let r = get_link_query(&mut [&mut txn], None, td.tag_query.clone());
    assert_eq!(r[0], td.link);

    // - Add the same link to the cache.
    insert_op(&mut cache_txn, td.base_op.clone());
    insert_op(&mut cache_txn, td.target_op.clone());
    insert_op(&mut cache_txn, td.create_link_op.clone());

    // - Check duplicates don't cause issues.
    insert_op(&mut cache_txn, td.create_link_op.clone());

    // - Add to the scratch
    insert_op_scratch(&mut scratch, td.base_op.clone());
    insert_op_scratch(&mut scratch, td.target_op.clone());
    insert_op_scratch(&mut scratch, td.create_link_op.clone());

    // - Check we can resolve this to a single link.
    let r = get_link_query(&mut [&mut cache_txn], Some(&scratch), td.base_query.clone());
    assert_eq!(r[0], td.link);
    assert_eq!(r.len(), 1);
    let r = get_link_query(
        &mut [&mut cache_txn, &mut txn],
        Some(&scratch),
        td.tag_query.clone(),
    );
    assert_eq!(r[0], td.link);
    assert_eq!(r.len(), 1);

    // - Insert a delete op.
    insert_op(&mut txn, td.delete_link_op.clone());

    let r = get_link_query(
        &mut [&mut cache_txn, &mut txn],
        Some(&scratch),
        td.tag_query.clone(),
    );
    // - We should not have any links now.
    assert!(r.is_empty())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_entry() {
    observability::test_run().ok();
    let mut scratch = Scratch::default();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut cache = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut cache, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let mut cache_txn = cache
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = EntryTestData::new();

    // - Create an entry on main db.
    insert_op(&mut txn, td.store_entry_op.clone());

    // - Check we get that header back.
    let r = get_entry_query(&mut [&mut txn], None, td.query.clone()).unwrap();
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);

    // - Create the same entry in the cache.
    insert_op(&mut cache_txn, td.store_entry_op.clone());
    // - Check duplicates is ok.
    insert_op(&mut cache_txn, td.store_entry_op.clone());

    // - Add to the scratch
    insert_op_scratch(&mut scratch, td.store_entry_op.clone());

    // - Get the entry from both stores and union the query results.
    let r = get_entry_query(
        &mut [&mut txn, &mut cache_txn],
        Some(&scratch),
        td.query.clone(),
    );
    // - Check it's the correct entry and header.
    let r = r.unwrap();
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);
    assert_eq!(*r.header(), *td.header.header());

    // - Delete the entry in the cache.
    insert_op(&mut cache_txn, td.delete_entry_header_op.clone());

    // - Get the entry from both stores and union the queries.
    let r = get_entry_query(
        &mut [&mut txn, &mut cache_txn],
        Some(&scratch),
        td.query.clone(),
    );
    // - There should be no live headers so resolving
    // returns no element.
    assert!(r.is_none());
}

/// Test that `insert_op` also inserts a header and potentially an entry
#[tokio::test(flavor = "multi_thread")]
async fn insert_op_equivalence() {
    observability::test_run().ok();
    let mut conn1 = Connection::open_in_memory().unwrap();
    let mut conn2 = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn1, None).unwrap();
    SCHEMA_CELL.initialize(&mut conn2, None).unwrap();

    let mut create_header = fixt!(Create);
    let create_entry = fixt!(Entry);
    let create_entry_hash = EntryHash::with_data_sync(&create_entry);
    create_header.entry_hash = create_entry_hash.clone();

    let sig = fixt!(Signature);
    let op = DhtOp::StoreEntry(
        sig.clone(),
        NewEntryHeader::Create(create_header.clone()),
        Box::new(create_entry.clone()),
    );
    let op = DhtOpHashed::from_content_sync(op);

    // Insert the op in 3 steps on conn1
    let mut txn1 = conn1
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let mut txn2 = conn2
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    insert_entry(&mut txn1, EntryHashed::from_content_sync(create_entry));
    insert_header(
        &mut txn1,
        SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::Create(create_header.clone())),
            fixt!(Signature),
        ),
    );
    insert_op_lite(&mut txn1, op.to_light(), op.as_hash().clone());

    // Insert the op in a single step on conn2
    insert_op(&mut txn2, op);

    txn1.commit().unwrap();
    txn2.commit().unwrap();

    // Query the DB on conn1
    let entries1: Vec<u8> = conn1
        .query_row("SELECT * FROM Entry", NO_PARAMS, |row| row.get("hash"))
        .unwrap();
    let headers1: Vec<u8> = conn1
        .query_row("SELECT * FROM Header", NO_PARAMS, |row| row.get("hash"))
        .unwrap();
    let ops1: Vec<u8> = conn1
        .query_row("SELECT * FROM DhtOp", NO_PARAMS, |row| row.get("hash"))
        .unwrap();

    // Query the DB on conn2
    let entries2: Vec<u8> = conn2
        .query_row("SELECT * FROM Entry", NO_PARAMS, |row| row.get("hash"))
        .unwrap();
    let headers2: Vec<u8> = conn2
        .query_row("SELECT * FROM Header", NO_PARAMS, |row| row.get("hash"))
        .unwrap();
    let ops2: Vec<u8> = conn2
        .query_row("SELECT * FROM DhtOp", NO_PARAMS, |row| row.get("hash"))
        .unwrap();

    assert_eq!(entries1, entries2);
    assert_eq!(headers1, headers2);
    assert_eq!(ops1, ops2);
}

macro_rules! sql_insert {
    ($txn:expr, $table:ident, { $($field:literal : $val:expr , )+ $(,)? }) => {{
        let table = stringify!($table);
        let fieldnames = &[ $( { $field } ,)+ ].join(",");
        let fieldvars = &[ $( { format!(":{}", $field) } ,)+ ].join(",");
        let sql = format!("INSERT INTO {} ({}) VALUES ({})", table, fieldnames, fieldvars);
        $txn.execute_named(&sql, &[$(
            (format!(":{}", $field).as_str(), &$val as &dyn holochain_sqlite::rusqlite::ToSql),
        )+])
    }};
}

#[derive(Debug, Clone)]
struct LinkQuery {
    base: EntryHash,
    zome_id: ZomeId,
    tag: Option<LinkTag>,
    create_string: String,
    delete_string: String,
}
#[derive(Debug, Clone)]
struct GetQuery(EntryHash);

impl GetQuery {
    fn create_query() -> &'static str {
        "
            SELECT Header.blob AS header_blob FROM DhtOp
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :store_entry
            AND
            DhtOp.basis_hash = :entry_hash
        "
    }

    fn delete_query() -> &'static str {
        "
        SELECT Header.blob AS header_blob FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :delete
        AND
        DhtOp.basis_hash = :entry_hash
        "
    }

    fn create_params(&self) -> Vec<Params> {
        let params = named_params! {
            ":store_entry": DhtOpType::StoreEntry,
            ":entry_hash": self.0,
        };
        params.to_vec()
    }

    fn delete_params(&self) -> Vec<Params> {
        let params = named_params! {
            ":delete": DhtOpType::RegisterDeletedEntryHeader,
            ":entry_hash": self.0,
        };
        params.to_vec()
    }
}

impl LinkQuery {
    fn new(base: EntryHash, zome_id: ZomeId, tag: Option<LinkTag>) -> Self {
        Self {
            base,
            zome_id,
            create_string: Self::create_query_string(tag.is_some()),
            delete_string: Self::delete_query_string(tag.is_some()),
            tag,
        }
    }

    fn base(base: EntryHash, zome_id: ZomeId) -> Self {
        Self::new(base, zome_id, None)
    }

    fn tag(base: EntryHash, zome_id: ZomeId, tag: LinkTag) -> Self {
        Self::new(base, zome_id, Some(tag))
    }

    fn create_query(&self) -> &str {
        &self.create_string
    }

    fn delete_query(&self) -> &str {
        &self.delete_string
    }

    fn common_query_string() -> &'static str {
        "
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :create
            AND
            Header.base_hash = :base_hash
            AND
            Header.zome_id = :zome_id
        "
    }
    fn create_query_string(tag: bool) -> String {
        let s = format!(
            "
            SELECT Header.blob AS header_blob FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        Self::add_tag(s, tag)
    }
    fn add_tag(q: String, tag: bool) -> String {
        if tag {
            format!(
                "{}
            AND
            Header.tag = :tag",
                q
            )
        } else {
            q
        }
    }
    fn delete_query_string(tag: bool) -> String {
        let sub_create_query = format!(
            "
            SELECT Header.hash FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        let sub_create_query = Self::add_tag(sub_create_query, tag);
        let delete_query = format!(
            "
            SELECT Header.blob AS header_blob FROM DhtOp
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :delete
            AND
            Header.create_link_hash IN ({})
            ",
            sub_create_query
        );
        delete_query
    }

    fn create_params(&self) -> Vec<Params> {
        let mut params = named_params! {
            ":create": DhtOpType::RegisterAddLink,
            ":base_hash": self.base,
            ":zome_id": self.zome_id,
        }
        .to_vec();
        if self.tag.is_some() {
            params.extend(named_params! {
                ":tag": self.tag,
            });
        }
        params
    }

    fn delete_params(&self) -> Vec<Params> {
        let mut params = named_params! {
            ":create": DhtOpType::RegisterAddLink,
            ":delete": DhtOpType::RegisterRemoveLink,
            ":base_hash": self.base,
            ":zome_id": self.zome_id,
        }
        .to_vec();
        if self.tag.is_some() {
            params.extend(named_params! {
                ":tag": self.tag,
            });
        }
        params
    }
}

#[derive(Debug)]
struct PlaceHolderError;

impl From<holochain_sqlite::rusqlite::Error> for PlaceHolderError {
    fn from(e: holochain_sqlite::rusqlite::Error) -> Self {
        tracing::error!(?e);
        todo!()
    }
}
impl From<std::convert::Infallible> for PlaceHolderError {
    fn from(_: std::convert::Infallible) -> Self {
        unreachable!()
    }
}

type Params<'a> = (&'a str, &'a dyn holochain_sqlite::rusqlite::ToSql);

struct Maps<T> {
    creates: HashMap<HeaderHash, T>,
    deletes: HashSet<HeaderHash>,
}

impl<T> Maps<T> {
    fn new() -> Self {
        Self {
            creates: Default::default(),
            deletes: Default::default(),
        }
    }
}

type Transactions<'a, 'txn> = [&'a mut Transaction<'txn>];

trait Query: Clone {
    type State;
    type Output;
    fn create_query(&self) -> &str {
        ""
    }
    fn delete_query(&self) -> &str {
        ""
    }
    fn update_query(&self) -> &str {
        ""
    }
    fn create_params(&self) -> Vec<Params> {
        Vec::with_capacity(0)
    }
    fn delete_params(&self) -> Vec<Params> {
        Vec::with_capacity(0)
    }
    fn update_params(&self) -> Vec<Params> {
        Vec::with_capacity(0)
    }
    fn init_fold(&self) -> Result<Self::State, PlaceHolderError>;

    fn as_filter(&self) -> Box<dyn Fn(&Header) -> bool>;

    fn fold(
        &mut self,
        state: Self::State,
        header: SignedHeaderHashed,
    ) -> Result<Self::State, PlaceHolderError>;
    fn render(
        &mut self,
        state: Self::State,
        txns: &mut Transactions<'_, '_>,
    ) -> Result<Self::Output, PlaceHolderError>;

    fn run(
        &mut self,
        txns: &mut Transactions<'_, '_>,
        scratch: Option<&Scratch>,
    ) -> Result<Self::Output, PlaceHolderError> {
        let mut stmts: Vec<_> = txns
            .into_iter()
            .map(|txn| QueryStmt::new(txn, self.clone()))
            .collect();
        let iter = stmts.iter_mut().map(|stmt| Ok(stmt.iter()));
        let iter = fallible_iterator::convert(iter).flatten();
        let scratch = scratch.map(|s| s.filter(self.as_filter()).map_err(PlaceHolderError::from));
        let result = match scratch {
            Some(scratch) => {
                let iter = iter.chain(scratch);
                iter.fold(self.init_fold()?, |state, i| self.fold(state, i))?
            }
            None => iter.fold(self.init_fold()?, |state, i| self.fold(state, i))?,
        };
        drop(stmts);
        self.render(result, txns)
    }
}

impl Query for GetQuery {
    type State = Maps<SignedHeaderHashed>;
    type Output = Option<Element>;

    fn create_query(&self) -> &str {
        GetQuery::create_query()
    }

    fn delete_query(&self) -> &str {
        GetQuery::delete_query()
    }

    fn create_params(&self) -> Vec<Params> {
        self.create_params()
    }

    fn delete_params(&self) -> Vec<Params> {
        self.delete_params()
    }

    fn init_fold(&self) -> Result<Self::State, PlaceHolderError> {
        Ok(Maps::new())
    }

    fn as_filter(&self) -> Box<dyn Fn(&Header) -> bool> {
        let entry_filter = self.0.clone();
        let f = move |header: &Header| match header {
            Header::Create(Create { entry_hash, .. }) => *entry_hash == entry_filter,
            Header::Delete(Delete {
                deletes_entry_address,
                ..
            }) => *deletes_entry_address == entry_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn fold(
        &mut self,
        mut state: Self::State,
        shh: SignedHeaderHashed,
    ) -> Result<Self::State, PlaceHolderError> {
        let hash = shh.as_hash().clone();
        match shh.header() {
            Header::Create(_) => {
                if !state.deletes.contains(&hash) {
                    state.creates.insert(hash, shh);
                }
            }
            Header::Delete(delete) => {
                state.creates.remove(&delete.deletes_address);
                state.deletes.insert(delete.deletes_address.clone());
            }
            _ => panic!("TODO: Turn this into an error"),
        }
        Ok(state)
    }

    fn render(
        &mut self,
        state: Self::State,
        txns: &mut Transactions<'_, '_>,
    ) -> Result<Self::Output, PlaceHolderError> {
        // Choose an arbitrary header
        let header = state.creates.into_iter().map(|(_, v)| v).next();
        match header {
            Some(header) => {
                for txn in txns.into_iter() {
                    let entry_hash = header.header().entry_hash().unwrap();
                    let entry = txn.query_row_named(
                        "
                    SELECT Entry.blob AS entry_blob FROM Entry
                    WHERE hash = :entry_hash
                    ",
                        named_params! {
                            ":entry_hash": entry_hash.clone(),
                        },
                        |row| {
                            Ok(from_blob::<Entry>(
                                row.get(row.column_index("entry_blob")?)?,
                            ))
                        },
                    );
                    if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &entry {
                        continue;
                    } else {
                        // TODO: Handle this error.
                        let entry = entry.unwrap();
                        return Ok(Some(Element::new(header, Some(entry))));
                    }
                }
                panic!("TODO: Handle case where entry wasn't found but we had headers")
            }
            None => Ok(None),
        }
    }
}

impl Query for LinkQuery {
    type State = Maps<Link>;
    type Output = Vec<Link>;
    fn create_query(&self) -> &str {
        self.create_query()
    }

    fn delete_query(&self) -> &str {
        self.delete_query()
    }

    fn create_params(&self) -> Vec<Params> {
        self.create_params()
    }

    fn delete_params(&self) -> Vec<Params> {
        self.delete_params()
    }

    fn init_fold(&self) -> Result<Self::State, PlaceHolderError> {
        Ok(Maps::new())
    }

    fn as_filter(&self) -> Box<dyn Fn(&Header) -> bool> {
        let base_filter = self.base.clone();
        let zome_id_filter = self.zome_id.clone();
        let tag_filter = self.tag.clone();
        let f = move |header: &Header| match header {
            Header::CreateLink(CreateLink {
                base_address,
                zome_id,
                tag,
                ..
            }) => {
                *base_address == base_filter
                    && *zome_id == zome_id_filter
                    && tag_filter.as_ref().map(|t| tag == t).unwrap_or(true)
            }
            Header::DeleteLink(DeleteLink { base_address, .. }) => *base_address == base_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn fold(
        &mut self,
        mut state: Self::State,
        shh: SignedHeaderHashed,
    ) -> Result<Self::State, PlaceHolderError> {
        let (header, _) = shh.into_header_and_signature();
        let (header, hash) = header.into_inner();
        match header {
            Header::CreateLink(create_link) => {
                if !state.deletes.contains(&hash) {
                    state
                        .creates
                        .insert(hash, link_from_header(Header::CreateLink(create_link)));
                }
            }
            Header::DeleteLink(delete_link) => {
                state.creates.remove(&delete_link.link_add_address);
                state.deletes.insert(delete_link.link_add_address);
            }
            _ => panic!("TODO: Turn this into an error"),
        }
        Ok(state)
    }

    fn render(
        &mut self,
        state: Self::State,
        _: &mut Transactions<'_, '_>,
    ) -> Result<Self::Output, PlaceHolderError> {
        Ok(state.creates.into_iter().map(|(_, v)| v).collect())
    }
}

struct QueryStmt<'stmt, Q: Query> {
    create_stmt: Statement<'stmt>,
    delete_stmt: Statement<'stmt>,
    query: Q,
}

impl<'stmt, 'iter, Q: Query> QueryStmt<'stmt, Q> {
    fn new(txn: &'stmt mut Transaction, query: Q) -> Self {
        let create_stmt = txn.prepare(&query.create_query()).unwrap();
        let delete_stmt = txn.prepare(&query.delete_query()).unwrap();
        Self {
            create_stmt,
            delete_stmt,
            query,
        }
    }
    fn iter(
        &'iter mut self,
    ) -> impl FallibleIterator<Item = SignedHeaderHashed, Error = PlaceHolderError> + 'iter {
        let creates = self
            .create_stmt
            .query_and_then_named(&self.query.create_params(), row_to_header)
            .unwrap();

        let deletes = self
            .delete_stmt
            .query_and_then_named(&self.query.delete_params(), row_to_header)
            .unwrap();
        let creates = fallible_iterator::convert(creates);
        let deletes = fallible_iterator::convert(deletes);
        creates.chain(deletes)
    }
}

fn row_to_header(row: &Row) -> Result<SignedHeaderHashed, PlaceHolderError> {
    let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
    let SignedHeader(header, signature) = header;
    let header = HeaderHashed::from_content_sync(header);
    let shh = SignedHeaderHashed::with_presigned(header, signature);
    Ok(shh)
}

fn get_link_query<'a, 'b: 'a>(
    txns: &mut [&'a mut Transaction<'b>],
    scratch: Option<&Scratch>,
    mut query: LinkQuery,
) -> Vec<Link> {
    query.run(txns, scratch).unwrap()
}

fn get_entry_query<'a, 'b: 'a>(
    txns: &mut [&'a mut Transaction<'b>],
    scratch: Option<&Scratch>,
    mut query: GetQuery,
) -> Option<Element> {
    query.run(txns, scratch).unwrap()
}

fn link_from_header(header: Header) -> Link {
    let hash = HeaderHash::with_data_sync(&header);
    match header {
        Header::CreateLink(header) => Link {
            target: header.target_address,
            timestamp: header.timestamp,
            tag: header.tag,
            create_link_hash: hash,
        },
        _ => panic!("TODO: handle this properly"),
    }
}

fn insert_op_scratch(scratch: &mut Scratch, op: DhtOpHashed) {
    let (op, _) = op.into_inner();
    let op_light = op.to_light();
    let header = op.header();
    let signature = op.signature().clone();
    if let Some(entry) = op.entry() {
        let _entry_hashed =
            EntryHashed::with_pre_hashed(entry.clone(), header.entry_hash().unwrap().clone());
        // TODO: Should we store the entry somewhere?
    }
    let header_hashed = HeaderHashed::with_pre_hashed(header, op_light.header_hash().to_owned());
    let header_hashed = SignedHeaderHashed::with_presigned(header_hashed, signature);
    scratch.add_header(header_hashed);
}

fn insert_op(txn: &mut Transaction, op: DhtOpHashed) {
    let (op, hash) = op.into_inner();
    let op_light = op.to_light();
    let header = op.header();
    let signature = op.signature().clone();
    if let Some(entry) = op.entry() {
        let entry_hashed =
            EntryHashed::with_pre_hashed(entry.clone(), header.entry_hash().unwrap().clone());
        insert_entry(txn, entry_hashed);
    }
    let header_hashed = HeaderHashed::with_pre_hashed(header, op_light.header_hash().to_owned());
    let header_hashed = SignedHeaderHashed::with_presigned(header_hashed, signature);
    insert_header(txn, header_hashed);
    insert_op_lite(txn, op_light, hash);
}

fn insert_op_lite(txn: &mut Transaction, op_lite: DhtOpLight, hash: DhtOpHash) {
    let header_hash = op_lite.header_hash().clone();
    let basis = op_lite.dht_basis().to_owned();
    sql_insert!(txn, DhtOp, {
        "hash": hash,
        "type": op_lite.get_type(),
        "basis_hash": basis,
        "header_hash": header_hash,
        "is_authored": 1,
        "require_receipt": 0,
        "blob": to_blob(op_lite),
    })
    .unwrap();
}

fn insert_header(txn: &mut Transaction, header: SignedHeaderHashed) {
    let (header, signature) = header.into_header_and_signature();
    let (header, hash) = header.into_inner();
    let header_type: HeaderTypeSql = header.header_type().into();
    let header_seq = header.header_seq();
    match header {
        Header::CreateLink(create_link) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "base_hash": create_link.base_address.clone(),
                "zome_id": create_link.zome_id.index() as u32,
                "tag": create_link.tag.clone(),
                "blob": to_blob(SignedHeader::from((Header::CreateLink(create_link), signature))),
            })
            .unwrap();
        }
        Header::DeleteLink(delete_link) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "create_link_hash": delete_link.link_add_address.clone(),
                "blob": to_blob(SignedHeader::from((Header::DeleteLink(delete_link), signature))),
            })
            .unwrap();
        }
        Header::Create(create) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "entry_hash": create.entry_hash.clone(),
                "blob": to_blob(SignedHeader::from((Header::Create(create), signature))),
            })
            .unwrap();
        }
        Header::Delete(delete) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "deletes_entry_hash": delete.deletes_entry_address.clone(),
                "deletes_header_hash": delete.deletes_address.clone(),
                "blob": to_blob(SignedHeader::from((Header::Delete(delete), signature))),
            })
            .unwrap();
        }
        _ => todo!(),
    }
}

#[derive(Debug, Clone, derive_more::Display, derive_more::From, derive_more::Into)]
pub struct HeaderTypeSql(HeaderType);

impl_to_sql_via_display!(HeaderTypeSql);

/// Just the name of the EntryType
#[derive(derive_more::Display)]
enum EntryTypeName {
    Agent,
    App,
    CapClaim,
    CapGrant,
}

impl_to_sql_via_display!(EntryTypeName);

fn insert_entry(txn: &mut Transaction, entry: EntryHashed) {
    let (entry, hash) = entry.into_inner();
    sql_insert!(txn, Entry, {
        "hash": hash,
        "type": EntryTypeName::from(&entry) ,
        "blob": to_blob(entry),
    })
    .unwrap();
}

fn to_blob<T: Serialize + Debug>(t: T) -> Vec<u8> {
    holochain_serialized_bytes::encode(&t).unwrap()
}

fn from_blob<T: DeserializeOwned + Debug>(blob: Vec<u8>) -> T {
    holochain_serialized_bytes::decode(&blob).unwrap()
}

impl From<&Entry> for EntryTypeName {
    fn from(e: &Entry) -> Self {
        match e {
            Entry::Agent(_) => Self::Agent,
            Entry::App(_) => Self::App,
            Entry::CapClaim(_) => Self::CapClaim,
            Entry::CapGrant(_) => Self::CapGrant,
        }
    }
}
