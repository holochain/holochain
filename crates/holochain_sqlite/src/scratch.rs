use std::convert::Infallible;

use fallible_iterator::FallibleIterator;
use holochain_zome_types::prelude::*;

/// The "scratch" is an in-memory space to stage Headers to be committed at the
/// end of the CallZome workflow.
///
/// This space must also be queryable: specifically, it needs to be combined
/// into queries into the database which return Headers. This is done by
/// a simple filter on the scratch space, and then chaining that iterator
/// onto the iterators over the Headers in the database(s) produced by the
/// Cascade.
pub struct Scratch(Vec<SignedHeaderHashed>);

impl Scratch {
    pub fn filter<'a, F: Fn(&'a Header) -> bool + 'a>(
        &'a self,
        f: F,
    ) -> impl FallibleIterator<Item = &'a SignedHeaderHashed, Error = Infallible> {
        fallible_iterator::convert(
            self.0
                .iter()
                .filter(move |shh| f(shh.header_hashed().as_content()))
                .map(Ok),
        )
    }
}

#[test]
fn test_multiple_in_memory() {
    use rusqlite::*;

    // blank string means "temporary database", which typically resides in
    // memory but can be flushed to disk if sqlite is under memory pressure
    let mut m1 = Connection::open("").unwrap();
    let mut m2 = Connection::open("").unwrap();

    let schema = "
CREATE TABLE mytable (
    x INTEGER PRIMARY KEY
);
    ";

    m1.execute(schema, NO_PARAMS).unwrap();
    m2.execute(schema, NO_PARAMS).unwrap();

    let num = m1
        .execute("INSERT INTO mytable (x) VALUES (1)", NO_PARAMS)
        .unwrap();
    assert_eq!(num, 1);

    let xs1: Vec<u16> = m1
        .transaction()
        .unwrap()
        .prepare_cached("SELECT x FROM mytable")
        .unwrap()
        .query_map(NO_PARAMS, |row| Ok(dbg!(row.get(0))?))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let xs2: Vec<u16> = m2
        .transaction()
        .unwrap()
        .prepare_cached("SELECT * FROM mytable")
        .unwrap()
        .query_map(NO_PARAMS, |row| Ok(row.get(0)?))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(xs1, vec![1]);
    assert!(xs2.is_empty());
}
