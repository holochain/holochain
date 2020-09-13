//! Traits and types for declaring how BufferedStores can work with
//! database fixture data

use crate::prelude::Readable;
use std::collections::BTreeSet;

/// Adds methods to load and read out test fixture data. For tests only!
pub trait LoadDbFixture {
    /// The item which this database uses for fixture data
    type FixtureItem;

    /// Write a FixtureItem to the DB.
    /// Should be used for tests only!
    fn write_test_datum(&mut self, data: Self::FixtureItem);

    /// Coerce the database to a state given by the fixture data.
    /// Should be used for tests only!
    fn write_test_data(&mut self, data: DbFixture<Self::FixtureItem>) {
        for datum in data {
            self.write_test_datum(datum)
        }
    }

    /// Retrieve data from DB as fixture data. NB: This may flush the scratch space!
    fn read_test_data<R: Readable>(&self, reader: &R) -> DbFixture<Self::FixtureItem>;
}

/// Type of data which can be written as a DB fixture
pub type DbFixture<T> = BTreeSet<T>;
