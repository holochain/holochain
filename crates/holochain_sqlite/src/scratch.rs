use std::convert::Infallible;

use fallible_iterator::FallibleIterator;

/// The "scratch" is an in-memory space to stage Headers to be committed at the
/// end of the CallZome workflow.
///
/// This space must also be queryable: specifically, it needs to be combined
/// into queries into the database which return Headers. This is done by
/// a simple filter on the scratch space, and then chaining that iterator
/// onto the iterators over the Headers in the database(s) produced by the
/// Cascade.
#[derive(Debug, Clone)]
pub struct Scratch<T>(Vec<T>);

pub struct FilteredScratch<T>(Vec<T>);

impl<T> Scratch<T>
where
    T: Clone,
{
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn add_item(&mut self, item: T) {
        self.0.push(item);
    }

    pub fn filter<'a, F: Fn(&'a T) -> bool + 'a>(
        &'a self,
        f: F,
    ) -> impl FallibleIterator<Item = T, Error = Infallible> + 'a {
        fallible_iterator::convert(
            self.0
                .iter()
                .filter(move |t| f(t))
                // TODO: @freesig Maybe this is a bad idea? Not sure yet.
                .cloned()
                .map(Ok),
        )
    }

    pub fn as_filter(&self, f: impl Fn(&T) -> bool) -> FilteredScratch<T> {
        FilteredScratch(self.0.iter().filter(|&t| f(t)).cloned().collect())
    }
}

impl<T> FilteredScratch<T> {
    pub fn into_iter<'iter>(&'iter mut self) -> impl Iterator<Item = T> + 'iter {
        self.0.drain(..)
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
