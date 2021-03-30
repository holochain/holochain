use ::fixt::prelude::*;
use either::Either;
use fallible_iterator::FallibleIterator;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::ToSql;
use holochain_sqlite::rusqlite::{Transaction, NO_PARAMS};
use holochain_sqlite::{impl_to_sql_via_display, rusqlite::TransactionBehavior};
use holochain_sqlite::{rusqlite::Statement, scratch::Scratch};
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

        let base_query = LinkQuery::Base(base_hash.clone(), create_link.zome_id.clone());
        let tag_query = LinkQuery::Tag(
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

    // - Check we can resolve this to a single link.
    let r = get_link_query(&mut [&mut cache_txn], None, td.base_query.clone());
    assert_eq!(r[0], td.link);
    assert_eq!(r.len(), 1);
    let r = get_link_query(&mut [&mut cache_txn, &mut txn], None, td.tag_query.clone());
    assert_eq!(r[0], td.link);
    assert_eq!(r.len(), 1);

    // - Insert a delete op.
    insert_op(&mut txn, td.delete_link_op.clone());

    let r = get_link_query(&mut [&mut cache_txn, &mut txn], None, td.tag_query.clone());
    // - We should not have any links now.
    assert!(r.is_empty())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_entry() {
    observability::test_run().ok();
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

    // - Get the entry from both stores and union the query results.
    let r = get_entry_query(&mut [&mut txn, &mut cache_txn], None, td.query.clone());
    // - Check it's the correct entry and header.
    let r = r.unwrap();
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);
    assert_eq!(*r.header(), *td.header.header());

    // - Delete the entry in the cache.
    insert_op(&mut cache_txn, td.delete_entry_header_op.clone());

    // - Get the entry from both stores and union the queries.
    let r = get_entry_query(&mut [&mut txn, &mut cache_txn], None, td.query.clone());
    // - There should be no live headers so resolving
    // returns no element.
    assert!(r.is_none());
}

// TODO: This could fail if we got the header from a different store.
// Perhaps this should take `&mut [&mut Transaction]` so it can search all
// stores for the entry. Short circuiting on the first place the entry is found.
fn render_entry<'a, 'b: 'a>(
    txns: &mut [&'a mut Transaction<'b>],
    headers: Vec<SignedHeaderHashed>,
) -> Option<Element> {
    // Choose an arbitrary header
    let header = headers.into_iter().next();
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
                        ":entry_hash": entry_hash.clone().into_inner(),
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
                    return Some(Element::new(header, Some(entry)));
                }
            }
            panic!("TODO: Handle case where entry wasn't found but we had headers")
        }
        None => None,
    }
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
enum LinkQuery {
    Base(EntryHash, ZomeId),
    Tag(EntryHash, ZomeId, LinkTag),
}
#[derive(Debug, Clone)]
struct GetQuery(EntryHash);

impl GetQuery {
    fn create_query_string() -> &'static str {
        "
            SELECT Header.blob AS header_blob FROM DhtOp
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :store_entry
            AND
            DhtOp.basis_hash = :entry_hash
        "
    }

    fn delete_query_string() -> &'static str {
        "
        SELECT Header.blob AS header_blob FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :delete
        AND
        DhtOp.basis_hash = :entry_hash
        "
    }

    fn into_params(self) -> GetQueryParams {
        GetQueryParams {
            entry_hash: self.0.into_inner(),
        }
    }

    fn as_filter(&self) -> impl Fn(&Header) -> bool {
        let entry_hash_filter = self.0.clone();
        move |header| match header {
            Header::Create(Create { entry_hash, .. }) => *entry_hash == entry_hash_filter,
            Header::Delete(Delete {
                deletes_entry_address,
                ..
            }) => *deletes_entry_address == entry_hash_filter,
            _ => false,
        }
    }
}

impl LinkQuery {
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
    // TODO: These could be made lazy to avoid allocating.
    fn create_query_string(&self) -> String {
        let s = format!(
            "
            SELECT Header.blob AS header_blob FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        self.add_tag(s)
    }
    fn add_tag(&self, q: String) -> String {
        match self {
            LinkQuery::Base(_, _) => q,
            LinkQuery::Tag(_, _, _) => format!(
                "
                    {}
                    AND
                    Header.tag = :tag
                    ",
                q
            ),
        }
    }
    fn delete_query_string(&self) -> String {
        let sub_create_query = format!(
            "
            SELECT Header.hash FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        let sub_create_query = self.add_tag(sub_create_query);
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

    fn into_params(self) -> LinkQueryParams {
        match self {
            LinkQuery::Base(e, z) => LinkQueryParams {
                entry_hash: e.into_inner(),
                zome_id: z.index() as u32,
                tag: None,
            },
            LinkQuery::Tag(e, z, t) => LinkQueryParams {
                entry_hash: e.into_inner(),
                zome_id: z.index() as u32,
                tag: Some(to_blob(t)),
            },
        }
    }

    fn as_filter(&self) -> impl Fn(&Header) -> bool {
        let (base_filter, zome_id_filter, tag_filter) = match self {
            Self::Base(hash, zome_id) => (hash.clone(), zome_id.clone(), None),
            Self::Tag(hash, zome_id, tag) => (hash.clone(), zome_id.clone(), Some(tag.clone())),
        };
        move |header| match header {
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
        }
    }
}

struct LinkQueryParams {
    entry_hash: Vec<u8>,
    zome_id: u32,
    tag: Option<Vec<u8>>,
}

struct GetQueryParams {
    entry_hash: Vec<u8>,
}

impl LinkQueryParams {
    fn create_link(&self) -> Vec<(&str, &dyn ToSql)> {
        let mut params = named_params! {
            ":create": DhtOpType::RegisterAddLink,
            ":base_hash": self.entry_hash,
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

    fn delete_link(&self) -> Vec<(&str, &dyn ToSql)> {
        let mut params = named_params! {
            ":create": DhtOpType::RegisterAddLink,
            ":delete": DhtOpType::RegisterRemoveLink,
            ":base_hash": self.entry_hash,
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

impl GetQueryParams {
    fn create(&self) -> Vec<(&str, &dyn ToSql)> {
        let params = named_params! {
            ":store_entry": DhtOpType::StoreEntry,
            ":entry_hash": self.entry_hash,
        };
        params.to_vec()
    }

    fn delete(&self) -> Vec<(&str, &dyn ToSql)> {
        let params = named_params! {
            ":delete": DhtOpType::RegisterDeletedEntryHeader,
            ":entry_hash": self.entry_hash,
        };
        params.to_vec()
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

struct LinkQueryStmt<'stmt> {
    create_stmt: Statement<'stmt>,
    delete_stmt: Statement<'stmt>,
    params: LinkQueryParams,
}

struct GetQueryStmt<'stmt> {
    create_stmt: Statement<'stmt>,
    delete_stmt: Statement<'stmt>,
    params: GetQueryParams,
}

impl<'stmt, 'iter> LinkQueryStmt<'stmt> {
    fn new(txn: &'stmt mut Transaction, query: LinkQuery) -> Self {
        let create_stmt = txn.prepare(&query.create_query_string()).unwrap();
        let delete_stmt = txn.prepare(&query.delete_query_string()).unwrap();
        let params = query.into_params();
        Self {
            create_stmt,
            delete_stmt,
            params,
        }
    }
    fn iter(
        &'iter mut self,
    ) -> impl FallibleIterator<Item = SignedHeaderHashed, Error = PlaceHolderError> + 'iter {
        let creates = self
            .create_stmt
            .query_and_then_named(&self.params.create_link(), |row| {
                let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
                let SignedHeader(header, signature) = header;
                let header = HeaderHashed::from_content_sync(header);
                let shh = SignedHeaderHashed::with_presigned(header, signature);
                Result::<_, PlaceHolderError>::Ok(shh)
            })
            .unwrap();

        let deletes = self
            .delete_stmt
            .query_and_then_named(&self.params.delete_link(), |row| {
                let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
                let SignedHeader(header, signature) = header;
                let header = HeaderHashed::from_content_sync(header);
                let shh = SignedHeaderHashed::with_presigned(header, signature);
                Result::<_, PlaceHolderError>::Ok(shh)
            })
            .unwrap();
        let creates = fallible_iterator::convert(creates);
        let deletes = fallible_iterator::convert(deletes);
        creates.chain(deletes)
    }
}

impl<'stmt, 'iter> GetQueryStmt<'stmt> {
    fn new(txn: &'stmt mut Transaction, query: GetQuery) -> Self {
        let create_stmt = txn.prepare(GetQuery::create_query_string()).unwrap();
        let delete_stmt = txn.prepare(GetQuery::delete_query_string()).unwrap();
        let params = query.into_params();
        Self {
            create_stmt,
            delete_stmt,
            params,
        }
    }
    fn iter(
        &'iter mut self,
    ) -> impl FallibleIterator<Item = SignedHeaderHashed, Error = PlaceHolderError> + 'iter {
        let creates = self
            .create_stmt
            .query_and_then_named(&self.params.create(), |row| {
                let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
                let SignedHeader(header, signature) = header;
                let header = HeaderHashed::from_content_sync(header);
                let shh = SignedHeaderHashed::with_presigned(header, signature);
                Result::<_, PlaceHolderError>::Ok(shh)
            })
            .unwrap();

        let deletes = self
            .delete_stmt
            .query_and_then_named(&self.params.delete(), |row| {
                let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
                let SignedHeader(header, signature) = header;
                let header = HeaderHashed::from_content_sync(header);
                let shh = SignedHeaderHashed::with_presigned(header, signature);
                Result::<_, PlaceHolderError>::Ok(shh)
            })
            .unwrap();
        let creates = fallible_iterator::convert(creates);
        let deletes = fallible_iterator::convert(deletes);
        creates.chain(deletes)
    }
}

fn get_link_query<'a, 'b: 'a>(
    txns: &mut [&'a mut Transaction<'b>],
    scratch: Option<&Scratch>,
    query: LinkQuery,
) -> Vec<Link> {
    let mut stmts: Vec<_> = txns
        .into_iter()
        .map(|txn| LinkQueryStmt::new(txn, query.clone()))
        .collect();
    let iter = stmts.iter_mut().map(|stmt| Ok(stmt.iter()));
    let iter = fallible_iterator::convert(iter).flatten();
    let (creates, _) = iter
        .fold(
            (HashMap::new(), HashSet::new()),
            |(mut creates, mut deletes), shh| {
                let (header, _) = shh.into_header_and_signature();
                let (header, hash) = header.into_inner();
                match header {
                    Header::CreateLink(create_link) => {
                        if !deletes.contains(&hash) {
                            creates.insert(hash, link_from_header(Header::CreateLink(create_link)));
                        }
                    }
                    Header::DeleteLink(delete_link) => {
                        creates.remove(&delete_link.link_add_address);
                        deletes.insert(delete_link.link_add_address);
                    }
                    _ => panic!("TODO: Turn this into an error"),
                }
                Ok((creates, deletes))
            },
        )
        .unwrap();
    creates.into_iter().map(|(_, v)| v).collect()
}

fn get_entry_query<'a, 'b: 'a>(
    txns: &mut [&'a mut Transaction<'b>],
    scratch: Option<&Scratch>,
    query: GetQuery,
) -> Option<Element> {
    let mut stmts: Vec<_> = txns
        .into_iter()
        .map(|txn| GetQueryStmt::new(txn, query.clone()))
        .collect();
    let iters = stmts.iter_mut().map(|stmt| Ok(stmt.iter()));
    let _ = scratch.map(|s| {
        s.filter(query.as_filter());
        todo!("chain scratch iterator onto others")
    });
    // let iters = iters.chain(fallible_iterator::convert(
    //     scratch
    //         .map(|s| {
    //             Either::Right(Either::Left(std::iter::once(Ok(
    //                 s.filter(query.as_filter())
    //             ))))
    //         })
    //         .unwrap_or_else(|| Either::Right(Either::Right(std::iter::empty()))),
    // ));
    let iter = fallible_iterator::convert(iters).flatten();

    let (creates, _) = iter
        .fold(
            (HashMap::new(), HashSet::new()),
            |(mut creates, mut deletes), shh| {
                let hash = shh.as_hash().clone();
                match shh.header() {
                    Header::Create(_) => {
                        if !deletes.contains(&hash) {
                            creates.insert(hash, shh);
                        }
                    }
                    Header::Delete(delete) => {
                        creates.remove(&delete.deletes_address);
                        deletes.insert(delete.deletes_address.clone());
                    }
                    _ => panic!("TODO: Turn this into an error"),
                }
                Ok((creates, deletes))
            },
        )
        .unwrap();
    drop(stmts);
    // TODO: We really only need a single header here so we can probably avoid this collect.
    let headers = creates.into_iter().map(|(_, v)| v).collect();
    render_entry(txns, headers)
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
        "hash": hash.into_inner(),
        "type": op_lite.get_type(),
        "basis_hash": basis.into_inner(),
        "header_hash": header_hash.into_inner(),
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
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "base_hash": create_link.base_address.clone().into_inner(),
                "zome_id": create_link.zome_id.index() as u32,
                "tag": to_blob(create_link.tag.clone()),
                "blob": to_blob(SignedHeader::from((Header::CreateLink(create_link), signature))),
            })
            .unwrap();
        }
        Header::DeleteLink(delete_link) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "create_link_hash": delete_link.link_add_address.clone().into_inner(),
                "blob": to_blob(SignedHeader::from((Header::DeleteLink(delete_link), signature))),
            })
            .unwrap();
        }
        Header::Create(create) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "entry_hash": create.entry_hash.clone().into_inner(),
                "blob": to_blob(SignedHeader::from((Header::Create(create), signature))),
            })
            .unwrap();
        }
        Header::Delete(delete) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "deletes_entry_hash": delete.deletes_entry_address.clone().into_inner(),
                "deletes_header_hash": delete.deletes_address.clone().into_inner(),
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
        "hash": hash.into_inner(),
        "type": EntryTypeName::from(&entry),
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
