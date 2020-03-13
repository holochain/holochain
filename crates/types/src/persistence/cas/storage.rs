//! ContentAddressableStorage (CAS) is defined here as a trait, such that there could be various implementations,
//! such as the memory based, and file storage based implementations already in this code base.
//! ContentAddressableStorage is a way of reading and writing AddressableContent in a persistent data store.
//! A test suite for CAS is also implemented here.

use crate::{
    cas::content::{Address, AddressableContent, Content, ExampleAddressableContent},
    eav::{
        Attribute, EavFilter, EaviQuery, EntityAttributeValueIndex, EntityAttributeValueStorage,
        IndexFilter,
    },
    error::{PersistenceError, PersistenceResult},
    has_uuid::HasUuid,
    holochain_json_api::{
        error::JsonError,
        json::{JsonString, RawString},
    },
    regex::Regex,
    reporting::ReportStorage,
};
use objekt;
use std::{
    collections::{BTreeSet, HashMap},
    convert::{TryFrom, TryInto},
    fmt::{self, Debug},
    sync::{Arc, RwLock},
};

use uuid::Uuid;

pub trait AddContent: objekt::Clone + Send + Sync + Debug {
    /// adds AddressableContent to the ContentAddressableStorage by its Address as Content
    fn add(&self, content: &dyn AddressableContent) -> PersistenceResult<()>;
}

pub trait FetchContent: objekt::Clone + Send + Sync + Debug + ReportStorage + AddContent {
    /// true if the Address is in the Store, false otherwise.
    /// may be more efficient than retrieve depending on the implementation.
    fn contains(&self, address: &Address) -> PersistenceResult<bool>;
    /// returns Some AddressableContent if it is in the Store, else None
    /// AddressableContent::from_content() can be used to allow the compiler to infer the type
    /// @see the fetch implementation for ExampleCas in the cas module tests
    fn fetch(&self, address: &Address) -> PersistenceResult<Option<Content>>;
}

/// content addressable store (CAS)
/// implements storage in memory or persistently
/// anything implementing AddressableContent can be added and fetched by address
/// CAS is append only
pub trait ContentAddressableStorage:
    objekt::Clone + Send + Sync + Debug + AddContent + FetchContent + HasUuid + ReportStorage
{
}

clone_trait_object!(AddContent);
clone_trait_object!(FetchContent);
clone_trait_object!(ContentAddressableStorage);

impl PartialEq for dyn ContentAddressableStorage {
    fn eq(&self, other: &dyn ContentAddressableStorage) -> bool {
        self.get_id() == other.get_id()
    }
}

impl HasUuid for ExampleContentAddressableStorage {
    fn get_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}

impl<C: AddContent + FetchContent + ReportStorage + HasUuid> ContentAddressableStorage for C {}

#[derive(Clone, Debug)]
/// some struct to show an example ContentAddressableStorage implementation
/// this is a thread-safe wrapper around the non-thread-safe implementation below
/// @see ExampleContentAddressableStorageActor
pub struct ExampleContentAddressableStorage {
    content: Arc<RwLock<ExampleContentAddressableStorageContent>>,
}

impl ExampleContentAddressableStorage {
    pub fn new() -> Result<ExampleContentAddressableStorage, JsonError> {
        Ok(ExampleContentAddressableStorage {
            content: Arc::new(RwLock::new(ExampleContentAddressableStorageContent::new())),
        })
    }
}

pub fn test_content_addressable_storage() -> ExampleContentAddressableStorage {
    ExampleContentAddressableStorage::new().expect("could not build example cas")
}

impl AddContent for ExampleContentAddressableStorage {
    fn add(&self, content: &dyn AddressableContent) -> PersistenceResult<()> {
        self.content
            .write()
            .unwrap()
            .unthreadable_add(&content.address(), &content.content())
            .map_err(|err| {
                let e: PersistenceError = err.into();
                e
            })
    }
}

impl FetchContent for ExampleContentAddressableStorage {
    fn contains(&self, address: &Address) -> PersistenceResult<bool> {
        self.content
            .read()
            .unwrap()
            .unthreadable_contains(address)
            .map_err(|err| err.into())
    }

    fn fetch(&self, address: &Address) -> PersistenceResult<Option<Content>> {
        Ok(self.content.read()?.unthreadable_fetch(address)?)
    }
}

impl ReportStorage for ExampleContentAddressableStorage {}

#[derive(Debug, Default)]
/// Not thread-safe CAS implementation with a HashMap
pub struct ExampleContentAddressableStorageContent {
    storage: HashMap<Address, Content>,
}

impl ExampleContentAddressableStorageContent {
    pub fn new() -> ExampleContentAddressableStorageContent {
        Default::default()
    }

    fn unthreadable_add(&mut self, address: &Address, content: &Content) -> Result<(), JsonError> {
        self.storage.insert(address.clone(), content.clone());
        Ok(())
    }

    fn unthreadable_contains(&self, address: &Address) -> Result<bool, JsonError> {
        Ok(self.storage.contains_key(address))
    }

    fn unthreadable_fetch(&self, address: &Address) -> Result<Option<Content>, JsonError> {
        Ok(self.storage.get(address).cloned())
    }
}

// A struct for our test suite that infers a type of ContentAddressableStorage
pub struct StorageTestSuite<T>
where
    T: ContentAddressableStorage,
{
    pub cas: T,
    // it is important that every cloned copy of any CAS has a consistent view to data
    pub cas_clone: T,
}

impl<T> StorageTestSuite<T>
where
    T: ContentAddressableStorage + 'static + Clone,
{
    pub fn new(cas: T) -> StorageTestSuite<T> {
        StorageTestSuite {
            cas_clone: cas.clone(),
            cas,
        }
    }

    // does round trip test that can infer two Addressable Content Types
    pub fn round_trip_test<Addressable, OtherAddressable>(
        self,
        content: Content,
        other_content: Content,
    ) where
        Addressable: AddressableContent + Clone + PartialEq + Debug,
        OtherAddressable: AddressableContent + Clone + PartialEq + Debug,
    {
        // based on associate type we call the right from_content function
        let addressable_content = Addressable::try_from_content(&content)
            .expect("could not create AddressableContent from Content");
        let other_addressable_content = OtherAddressable::try_from_content(&other_content)
            .expect("could not create AddressableContent from Content");

        // do things that would definitely break if cloning would show inconsistent data
        let both_cas = vec![self.cas.clone(), self.cas_clone.clone()];

        for cas in both_cas.iter() {
            assert_eq!(Ok(false), cas.contains(&addressable_content.address()));
            assert_eq!(Ok(None), cas.fetch(&addressable_content.address()));
            assert_eq!(
                Ok(false),
                cas.contains(&other_addressable_content.address())
            );
            assert_eq!(Ok(None), cas.fetch(&other_addressable_content.address()));
        }

        // round trip some AddressableContent through the ContentAddressableStorage
        assert_eq!(Ok(()), self.cas.add(&content));

        for cas in both_cas.iter() {
            assert_eq!(Ok(true), cas.contains(&content.address()));
            assert_eq!(Ok(false), cas.contains(&other_content.address()));
            assert_eq!(Ok(Some(content.clone())), cas.fetch(&content.address()));
        }

        // multiple types of AddressableContent can sit in a single ContentAddressableStorage
        // the safety of this is only as good as the hashing algorithm(s) used
        assert_eq!(Ok(()), self.cas_clone.add(&other_content));

        for cas in both_cas.iter() {
            assert_eq!(Ok(true), cas.contains(&content.address()));
            assert_eq!(Ok(true), cas.contains(&other_content.address()));
            assert_eq!(Ok(Some(content.clone())), cas.fetch(&content.address()));
            assert_eq!(
                Ok(Some(other_content.clone())),
                cas.fetch(&other_content.address())
            );
        }

        // show consistent view on data across threads
        /* TODO replace entry with some other type to fix the test
        let entry = test_entry_unique();

        // initially should not find entry
        let thread_cas = self.cas.clone();
        let thread_entry = entry.clone();
        let (tx1, rx1) = channel();
        thread::spawn(move || {
            assert_eq!(
                None,
                thread_cas
                    .fetch(&thread_entry.address())
                    .expect("could not fetch from cas")
            );
            tx1.send(true).unwrap();
        });

        // should be able to add an entry found in the next channel
        let mut thread_cas = self.cas.clone();
        let thread_entry = entry.clone();
        let (tx2, rx2) = channel();
        thread::spawn(move || {
            rx1.recv().unwrap();
            thread_cas
                .add(&thread_entry)
                .expect("could not add entry to cas");
            tx2.send(true).expect("could not kick off next thread");
        });

        let thread_cas = self.cas.clone();
        let thread_entry = entry.clone();
        let handle = thread::spawn(move || {
            rx2.recv().unwrap();
            assert_eq!(
                Some(thread_entry.clone()),
                thread_cas
                    .fetch(&thread_entry.address())
                    .expect("could not fetch from cas")
                    .map(|content| Entry::try_from(content).unwrap())
            )
        });

        handle.join().unwrap();
        */
    }
}

pub struct EavTestSuite;

#[derive(
    Debug, Hash, Clone, Serialize, Deserialize, DefaultJson, Eq, PartialEq, PartialOrd, Ord,
)]
pub enum ExampleLink {
    RemovedLink(String, String),
    LinkTag(String, String),
}

impl std::fmt::Display for ExampleLink {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExampleLink::LinkTag(link_type, tag) => write!(f, "link__{}__{}", link_type, tag),
            ExampleLink::RemovedLink(link_type, tag) => {
                write!(f, "removed_link__{}__{}", link_type, tag)
            }
        }
    }
}

lazy_static! {
    static ref LINK_REGEX: Regex =
        Regex::new(r"^link__(.*)__(.*)$").expect("This string literal is a valid regex");
    static ref REMOVED_LINK_REGEX: Regex =
        Regex::new(r"^removed_link__(.*)__(.*)$").expect("This string literal is a valid regex");
}

impl TryFrom<&str> for ExampleLink {
    type Error = PersistenceError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if LINK_REGEX.is_match(s) {
            let link_type = LINK_REGEX.captures(s)?.get(1)?.as_str().to_string();
            let link_tag = LINK_REGEX.captures(s)?.get(2)?.as_str().to_string();

            Ok(ExampleLink::LinkTag(link_type, link_tag))
        } else if REMOVED_LINK_REGEX.is_match(s) {
            let link_type = REMOVED_LINK_REGEX.captures(s)?.get(1)?.as_str().to_string();
            let link_tag = REMOVED_LINK_REGEX.captures(s)?.get(2)?.as_str().to_string();
            Ok(ExampleLink::RemovedLink(link_type, link_tag))
        } else {
            Err(PersistenceError::SerializationError(format!(
                "Not a properly example link: {}",
                s.to_string()
            )))
        }
    }
}

impl TryFrom<String> for ExampleLink {
    type Error = PersistenceError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.as_str().try_into()
    }
}

impl EavTestSuite {
    pub fn test_round_trip<A: Attribute>(
        eav_storage: impl EntityAttributeValueStorage<A> + Clone,
        entity_content: impl AddressableContent,
        attribute: A,
        value_content: impl AddressableContent,
    ) {
        let eav: EntityAttributeValueIndex<A> = EntityAttributeValueIndex::new(
            &entity_content.address(),
            &attribute,
            &value_content.address(),
        )
        .expect("Could create entityAttributeValue");

        let two_stores = vec![eav_storage.clone(), eav_storage.clone()];

        for store in two_stores.iter() {
            let query = EaviQuery::new(
                Some(entity_content.address()).into(),
                Some(attribute.clone()).into(),
                Some(value_content.address()).into(),
                IndexFilter::LatestByAttribute,
                None,
            );
            assert_eq!(
                BTreeSet::new(),
                store.fetch_eavi(&query).expect("could not fetch eav"),
            );
        }

        eav_storage.add_eavi(&eav).expect("could not add eav");
        let two_stores = vec![eav_storage.clone(), eav_storage];
        let mut expected = BTreeSet::new();
        expected.insert(eav);
        for eav_storage in two_stores.iter() {
            // some examples of constraints that should all return the eav
            for (e, a, v) in vec![
                // constrain all
                (
                    Some(entity_content.address()),
                    Some(attribute.clone()),
                    Some(value_content.address()),
                ),
                // open entity
                (None, Some(attribute.clone()), Some(value_content.address())),
                // open attribute
                (
                    Some(entity_content.address()),
                    None,
                    Some(value_content.address()),
                ),
                // open value
                (
                    Some(entity_content.address()),
                    Some(attribute.clone()),
                    None,
                ),
                // open
                (None, None, None),
            ] {
                assert_eq!(
                    expected,
                    eav_storage
                        .fetch_eavi(&EaviQuery::new(
                            e.into(),
                            a.into(),
                            v.into(),
                            IndexFilter::LatestByAttribute,
                            None
                        ))
                        .expect("could not fetch eav")
                );
            }
        }
    }
    pub fn test_one_to_many<A, AT: Attribute, S>(eav_storage: S, attribute: &AT)
    where
        A: AddressableContent + Clone,
        S: EntityAttributeValueStorage<AT>,
    {
        let foo_content = Content::from(RawString::from("foo"));
        let bar_content = Content::from(RawString::from("bar"));
        let baz_content = Content::from(RawString::from("baz"));

        let one = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        // it can reference itself, why not?
        let many_one = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        let many_two = A::try_from_content(&bar_content)
            .expect("could not create AddressableContent from Content");
        let many_three = A::try_from_content(&baz_content)
            .expect("could not create AddressableContent from Content");

        let mut expected = BTreeSet::new();
        for many in vec![many_one.clone(), many_two.clone(), many_three.clone()] {
            let eav = EntityAttributeValueIndex::new(&one.address(), attribute, &many.address())
                .expect("could not create EAV");
            eav_storage
                .add_eavi(&eav)
                .expect("could not add eav")
                .expect("could not add eav");
        }

        // throw an extra thing referencing many to show fetch ignores it
        let two = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        for many in vec![many_one.clone(), many_two.clone(), many_three.clone()] {
            let eavi = eav_storage
                .add_eavi(
                    &EntityAttributeValueIndex::new(&two.address(), attribute, &many.address())
                        .expect("Could not create eav"),
                )
                .expect("could not add eav")
                .expect("could not add eav");
            expected.insert(eavi);
        }

        // show the many results for one
        assert_eq!(
            expected,
            eav_storage
                .fetch_eavi(&EaviQuery::new(
                    Some(one.address()).into(),
                    Some(attribute.clone()).into(),
                    None.into(),
                    IndexFilter::LatestByAttribute,
                    None
                ))
                .expect("could not fetch eav")
        );

        // show one for the many results
        for many in vec![many_one, many_two, many_three] {
            let mut expected_one = BTreeSet::new();
            let eav =
                EntityAttributeValueIndex::new(&one.address(), &attribute.clone(), &many.address())
                    .expect("Could not create eav");
            expected_one.insert(eav);
            let fetch_set = eav_storage
                .fetch_eavi(&EaviQuery::new(
                    None.into(),
                    Some(attribute.clone()).into(),
                    Some(many.address()).into(),
                    IndexFilter::LatestByAttribute,
                    None,
                ))
                .expect("could not fetch eav");
            assert_eq!(fetch_set.clone().len(), expected_one.clone().len());
            fetch_set.iter().zip(&expected_one).for_each(|(a, b)| {
                assert_eq!(a.entity(), b.entity());
                assert_eq!(a.attribute(), b.attribute());
                assert_eq!(a.value(), a.value());
            });
        }
    }

    pub fn test_range<A, AT: Attribute, S>(eav_storage: S, attribute: &AT)
    where
        A: AddressableContent + Clone,
        S: EntityAttributeValueStorage<AT>,
    {
        let foo_content = Content::from(RawString::from("foo"));
        let bar_content = Content::from(RawString::from("bar"));

        let one = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        // it can reference itself, why not?
        let many_one = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        let many_two = A::try_from_content(&bar_content)
            .expect("could not create AddressableContent from Content");
        let mut expected_many_one = BTreeSet::new();
        let mut expected_many_two = BTreeSet::new();
        let mut expected_all_range = BTreeSet::new();
        let addresses = vec![many_one.address(), many_two.address()];

        //iterate 5 times
        (0..5).for_each(|s| {
            let alter_index = s % 2;
            let eav =
                EntityAttributeValueIndex::new(&addresses[alter_index], attribute, &one.address())
                    .expect("could not create EAV");
            let eavi = eav_storage
                .add_eavi(&eav)
                .expect("could not add eav")
                .expect("Could not get eavi option");
            if s % 2 == 0 {
                //insert many ones
                expected_many_one.insert(eavi.clone());
            } else {
                //insert many twos
                expected_many_two.insert(eavi.clone());
            }
            //insert every range
            if s > 1 {
                expected_all_range.insert(eavi);
            }
        });

        // get only many one values per specified range
        let index_query_many_one = IndexFilter::Range(
            Some(expected_many_one.iter().next().unwrap().index()),
            Some(expected_many_one.iter().last().unwrap().index()),
        );
        assert_eq!(
            expected_many_one,
            eav_storage
                .fetch_eavi(&EaviQuery::new(
                    Some(many_one.address()).into(),
                    Some(attribute.clone()).into(),
                    Some(one.address()).into(),
                    index_query_many_one,
                    None
                ))
                .unwrap()
        );

        // get only many two values per specified range
        let index_query_many_two = IndexFilter::Range(
            Some(expected_many_two.iter().next().unwrap().index()),
            Some(expected_many_two.iter().last().unwrap().index()),
        );
        assert_eq!(
            expected_many_two,
            eav_storage
                .fetch_eavi(&EaviQuery::new(
                    Some(many_two.address()).into(),
                    Some(attribute.clone()).into(),
                    Some(one.address()).into(),
                    index_query_many_two,
                    None
                ))
                .unwrap()
        );

        // get all values per specified range
        let index_query_all = IndexFilter::Range(
            Some(expected_all_range.iter().next().unwrap().index()),
            Some(expected_all_range.iter().last().unwrap().index()),
        );
        assert_eq!(
            expected_all_range,
            eav_storage
                .fetch_eavi(&EaviQuery::new(
                    None.into(),
                    Some(attribute.clone()).into(),
                    Some(one.address()).into(),
                    index_query_all,
                    None
                ))
                .unwrap()
        );
    }

    pub fn test_multiple_attributes<A, AT: Attribute, S>(eav_storage: S, attributes: Vec<AT>)
    where
        A: AddressableContent + Clone,
        S: EntityAttributeValueStorage<AT>,
    {
        let foo_content = Content::from(RawString::from("foo"));

        let one = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        // it can reference itself, why not?
        let many_one = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        let mut expected = BTreeSet::new();

        attributes.iter().for_each(|attribute| {
            let eav: EntityAttributeValueIndex<AT> =
                EntityAttributeValueIndex::new(&many_one.address(), attribute, &one.address())
                    .expect("could not create EAV");
            let eavi = eav_storage
                .add_eavi(&eav)
                .expect("could not add eav")
                .expect("Could not get eavi option");
            expected.insert(eavi);
        });

        let query = EaviQuery::new(
            Some(many_one.address()).into(),
            attributes.into(),
            EavFilter::default(),
            IndexFilter::LatestByAttribute,
            None,
        );

        // get only last value in set of prefix query
        let results = eav_storage.fetch_eavi(&query).unwrap();
        assert_eq!(1, results.len());

        assert_eq!(
            expected.iter().last().unwrap(),
            results.iter().last().unwrap()
        );

        //add another value just to prove we get last of prefix
        let first_eav = expected.iter().next().unwrap();
        //timestamp in constructor generates new time
        let new_eav = EntityAttributeValueIndex::new(
            &first_eav.entity(),
            &first_eav.attribute(),
            &first_eav.value(),
        )
        .expect("could not create EAV");
        let new_eavi = eav_storage.add_eavi(&new_eav);
        // get only last value in set of prefix
        let results = eav_storage.fetch_eavi(&query).unwrap();
        assert_eq!(&new_eavi.unwrap().unwrap(), results.iter().last().unwrap())
    }

    pub fn test_many_to_one<A, AT: Attribute, S>(eav_storage: S, attribute: &AT)
    where
        A: AddressableContent + Clone,
        S: EntityAttributeValueStorage<AT>,
    {
        let foo_content = Content::from(RawString::from("foo"));
        let bar_content = Content::from(RawString::from("bar"));
        let baz_content = Content::from(RawString::from("baz"));

        let one = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");

        // it can reference itself, why not?
        let many_one = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        let many_two = A::try_from_content(&bar_content)
            .expect("could not create AddressableContent from Content");
        let many_three = A::try_from_content(&baz_content)
            .expect("could not create AddressableContent from Content");

        let mut expected = BTreeSet::new();
        for many in vec![many_one.clone(), many_two.clone(), many_three.clone()] {
            let eav = EntityAttributeValueIndex::new(&many.address(), attribute, &one.address())
                .expect("could not create EAV");
            eav_storage
                .add_eavi(&eav)
                .expect("could not add eav")
                .expect("Could not get eavi option");
        }

        // throw an extra thing referenced by many to show fetch ignores it
        let two = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        for many in vec![many_one.clone(), many_two.clone(), many_three.clone()] {
            let eavi = eav_storage
                .add_eavi(
                    &EntityAttributeValueIndex::new(&many.address(), attribute, &two.address())
                        .expect("Could not create eav"),
                )
                .expect("could not add eav")
                .expect("could not add eav");
            expected.insert(eavi);
        }

        let query = EaviQuery::new(
            EavFilter::default(),
            EavFilter::single(attribute.clone()),
            EavFilter::single(one.address()),
            IndexFilter::LatestByAttribute,
            None,
        );
        // show the many referencing one
        assert_eq!(
            expected,
            eav_storage.fetch_eavi(&query).expect("could not fetch eav"),
        );

        // show one for the many results
        for many in vec![many_one, many_two, many_three] {
            let mut expected_one = BTreeSet::new();
            let eav =
                EntityAttributeValueIndex::new(&many.address(), &attribute.clone(), &one.address())
                    .expect("Could not create eav");
            expected_one.insert(eav);
            let fetch_set = eav_storage
                .fetch_eavi(&EaviQuery::new(
                    Some(many.address()).into(),
                    Some(attribute.clone()).into(),
                    None.into(),
                    IndexFilter::LatestByAttribute,
                    None,
                ))
                .expect("could not fetch eav");
            assert_eq!(fetch_set.clone().len(), expected_one.clone().len());
            fetch_set.iter().zip(&expected_one).for_each(|(a, b)| {
                assert_eq!(a.entity(), b.entity());
                assert_eq!(a.attribute(), b.attribute());
                assert_eq!(a.value(), a.value());
            });
        }
    }

    //this tests tombstone functionality in the sense of , if there is a tombstone variable set that matches the predicate it should take precedent over everything else that is found
    //and if there isn't it should get the latest. This test will test both scenarios in which a tombstone is set and a match is found and a tombstone is set and a match is not found.
    //no need to test the case in which a tombstone is not set because it is has been applied in previous tests already
    pub fn test_tombstone<A, S>(eav_storage: S)
    where
        A: AddressableContent + Clone,
        S: EntityAttributeValueStorage<ExampleLink>,
    {
        let foo_content = Content::from(RawString::from("foo"));
        let bar_content = Content::from(RawString::from("bar"));
        let baz_content = Content::from(RawString::from("baz"));

        let one = A::try_from_content(&foo_content)
            .expect("could not create AddressableContent from Content");
        let two = A::try_from_content(&bar_content)
            .expect("could not create AddressableContent from Content");
        let three = A::try_from_content(&baz_content)
            .expect("could not create AddressableContent from Content");

        //set the value that should take precedence over everything when we set our tombstone
        let tombstone_attribute = ExampleLink::RemovedLink("c".into(), "c".into());
        let mut expected_other_tombstone = BTreeSet::new();
        let mut expected_tombstone = BTreeSet::new();
        let mut expected_tombstone_not_found = BTreeSet::new();
        //this is our test data
        vec!["ac", "bd", "c", "dc", "e"].iter().for_each(|s| {
            let str = String::from(*s);
            //for each test data that comes through, we should create an EAV with link_tag over it
            let eav = EntityAttributeValueIndex::new_with_index(
                &one.address(),
                &ExampleLink::LinkTag(str.clone(), str.clone()),
                &two.address(),
                0,
            )
            .expect("could not create EAV");

            //add to our EAV storage
            let expected_eav = eav_storage
                .add_eavi(&eav)
                .expect("could not add eav")
                .expect("Could not get eavi option");
            if *s == "c" {
                //when we reach C we are going to add a remove_link EAVI
                let eav_remove = EntityAttributeValueIndex::new_with_index(
                    &one.address(),
                    &ExampleLink::RemovedLink(str.clone(), str.clone()),
                    &two.address(),
                    0,
                )
                .expect("could not create EAV");

                //right after we are also going to add a LinkTag with the same C data just to diversify the data
                let new_remove_eav = eav_storage
                    .add_eavi(&eav_remove)
                    .expect("could not add eav")
                    .expect("Could not get eavi option");

                let eav_remove_2 = EntityAttributeValueIndex::new_with_index(
                    &one.address(),
                    &ExampleLink::RemovedLink(str.clone(), str),
                    &three.address(),
                    new_remove_eav.index(),
                )
                .expect("could not create EAV");

                let new_remove_eav_2 = eav_storage
                    .add_eavi(&eav_remove_2)
                    .expect("could not add eav")
                    .expect("Could not get eavi option");

                expected_tombstone.insert(new_remove_eav);
                expected_other_tombstone.insert(new_remove_eav_2);
                eav_storage
                    .add_eavi(&eav)
                    .expect("could not add eav")
                    .expect("Could not get eavi option");
            } else if *s == "e" {
                //this is the last value we expect out of query
                expected_tombstone_not_found.insert(expected_eav);
            } else {
                ()
            }
        });

        //get from the eavi, if tombstone is found return that as priority
        let expected_attribute = Some(tombstone_attribute);
        println!("expected other tombstone {:?}", expected_other_tombstone);

        //this assert is supposed to return RemovedLink::("c","c") as since we have set it in our tombstone it should take precedence over everything
        assert_eq!(
            expected_tombstone,
            eav_storage
                .fetch_eavi(&EaviQuery::new(
                    Some(one.address()).into(),
                    None.into(),
                    Some(two.address()).into(),
                    IndexFilter::LatestByAttribute,
                    Some(expected_attribute.clone().into())
                )) // this fetch eavi sets a tombstone on remove_link(c,c) which means It will catch the tombstone on remove_link
                .unwrap()
        );

        //this tests if complex predicates are able to be matched on and return tombstone
        assert_eq!(
            expected_tombstone,
            eav_storage
                .fetch_eavi(&EaviQuery::new(
                    Some(one.address()).into(),
                    EavFilter::predicate(move |attr| {
                        match attr {
                            ExampleLink::RemovedLink(link_type, tag)
                            | ExampleLink::LinkTag(link_type, tag) => {
                                link_type.contains('c') || tag.contains('c')
                            }
                        }
                    }),
                    Some(two.address()).into(),
                    IndexFilter::LatestByAttribute,
                    Some(EavFilter::predicate(move |attr| {
                        match attr {
                            ExampleLink::RemovedLink(_, _) => true,
                            _ => false,
                        }
                    }))
                )) // this fetch eavi sets a tombstone on remove_link(c,c) which means It will catch the tombstone on remove_link
                .unwrap()
        );

        assert_eq!(
            expected_other_tombstone,
            eav_storage
                .fetch_eavi(&EaviQuery::new(
                    Some(one.address()).into(),
                    None.into(),
                    Some(three.address()).into(),
                    IndexFilter::LatestByAttribute,
                    Some(expected_attribute.into())
                )) // this fetch eavi sets a tombstone on remove_link(c,c) which means It will catch the tombstone on remove_link
                .unwrap()
        );

        //this assert willnot return RemovedLink("C","C") because even though the tombstone was set it was not found so we default to the latest
        let expected_last_attribute = Some(ExampleLink::RemovedLink("e".into(), "e".into()));
        assert_eq!(
            expected_tombstone_not_found,
            eav_storage
                .fetch_eavi(&EaviQuery::new(
                    Some(one.address()).into(),
                    None.into(),
                    Some(two.address()).into(),
                    IndexFilter::LatestByAttribute,
                    Some(expected_last_attribute.into())
                ))
                .unwrap()
        );
    }
}

pub struct CasBencher;

impl CasBencher {
    pub fn random_addressable_content() -> ExampleAddressableContent {
        let s: String = (0..4).map(|_| rand::random::<char>()).collect();
        ExampleAddressableContent::try_from_content(&RawString::from(s).into()).unwrap()
    }

    pub fn bench_add(b: &mut test::Bencher, store: impl ContentAddressableStorage) {
        b.iter(|| store.add(&CasBencher::random_addressable_content()))
    }

    pub fn bench_fetch(b: &mut test::Bencher, store: impl ContentAddressableStorage) {
        // add some values to make it realistic
        for _ in 0..100 {
            store
                .add(&CasBencher::random_addressable_content())
                .unwrap();
        }

        let test_content = CasBencher::random_addressable_content();
        store.add(&test_content).unwrap();

        b.iter(|| store.fetch(&test_content.address()))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::persistence::cas::{
        content::{ExampleAddressableContent, OtherExampleAddressableContent},
        storage::{test_content_addressable_storage, StorageTestSuite},
    };
    use holochain_json_api::json::{JsonString, RawString};

    /// show that content of different types can round trip through the same storage
    #[test]
    fn example_content_round_trip_test() {
        let test_suite = StorageTestSuite::new(test_content_addressable_storage());
        test_suite.round_trip_test::<ExampleAddressableContent, OtherExampleAddressableContent>(
            JsonString::from(RawString::from("foo")),
            JsonString::from(RawString::from("bar")),
        );
    }
}
