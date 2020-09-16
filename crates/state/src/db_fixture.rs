//! Traits and types for declaring how BufferedStores can work with
//! database fixture data

use crate::prelude::{Readable, Writer};
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

    /// Retrieve data from DB as fixture data.
    fn read_test_data<R: Readable>(&self, reader: &R) -> DbFixture<Self::FixtureItem>;

    /// Retrieve data from DB as fixture data, using a write transaction.
    /// This is used for database types where we don't have a way of iterating over
    /// both scratch and persisted data, and where we must apply the scratch
    /// space to the persisted data in order to get an accurate combined view.
    ///
    /// NB: this does *not* need to modify anything, as we can simply drop the
    /// Writer without committing it.
    fn read_test_data_mut(&mut self, writer: &mut Writer) -> DbFixture<Self::FixtureItem> {
        self.read_test_data(writer)
    }
}

/// Type of data which can be written as a DB fixture
pub type DbFixture<T> = BTreeSet<T>;
