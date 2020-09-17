use super::*;
use holochain_state::db_fixture::{DbFixture, LoadDbFixture};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ElementBufFixtureItem<P: PrefixType> {
    PublicEntries(<EntryCas<P> as LoadDbFixture>::FixtureItem),
    PrivateEntries(<EntryCas<P> as LoadDbFixture>::FixtureItem),
    Headers(<HeaderCas<P> as LoadDbFixture>::FixtureItem),
}

impl<P: PrefixType> LoadDbFixture for ElementBuf<P> {
    type FixtureItem = ElementBufFixtureItem<P>;

    fn write_test_datum(&mut self, datum: Self::FixtureItem) {
        match datum {
            Self::FixtureItem::Headers(d) => self.headers.write_test_datum(d),
            Self::FixtureItem::PublicEntries(d) => self.public_entries.write_test_datum(d),
            Self::FixtureItem::PrivateEntries(d) => self
                .private_entries
                .as_mut()
                .expect("Using private entry fixture data when private entry access is disabled")
                .write_test_datum(d),
        }
    }

    fn read_test_data<R: Readable>(&self, reader: &R) -> DbFixture<Self> {
        let headers = self
            .headers
            .iter_fail(reader)
            .expect("Couldn't iterate when gathering fixture data")
            .map(|i| Ok(Self::FixtureItem::Headers(i)));

        let public_entries = self
            .public_entries
            .iter_fail(reader)
            .expect("Couldn't iterate when gathering fixture data")
            .map(|i| Ok(Self::FixtureItem::PublicEntries(i)));

        if let Some(buf) = &self.private_entries {
            let private_entries = buf
                .iter_fail(reader)
                .expect("Couldn't iterate when gathering fixture data")
                .map(|i| Ok(Self::FixtureItem::PrivateEntries(i)));

            headers
                .chain(public_entries)
                .chain(private_entries)
                .collect()
                .expect("Couldn't collect fixture data")
        } else {
            headers
                .chain(public_entries)
                .collect()
                .expect("Couldn't collect fixture data")
        }
    }
}
