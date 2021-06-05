
use hdk::prelude::*;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Failed to parse from element")]
    FromElement,
}

impl From<Error> for WasmError {
    fn from(e: Error) -> Self {
        Self::Guest(e.to_string())
    }
}

fn element_to_entry<'a, O>(element: &'a Element) -> Result<O, Error> where O: TryFrom<&'a Entry> {
    Ok(match element.entry() {
        ElementEntry::Present(serialized) => match O::try_from(serialized) {
            Ok(o) => o,
            Err(_) => return Err(Error::FromElement),
        },
        _ => return Err(Error::FromElement),
    })
}

#[hdk_entry(id = "something")]
#[derive(Clone)]
struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

#[hdk_entry(id = "entry_reference")]
struct EntryReference(EntryHash);

impl TryFrom<&Element> for EntryReference {
    type Error = Error;
    fn try_from(element: &Element) -> Result<Self, Self::Error> {
        element_to_entry(element)
    }
}

impl EntryReference {
    fn into_inner(self) -> EntryHash {
        self.0
    }
}

#[hdk_entry(id = "header_reference")]
struct HeaderReference(HeaderHash);

impl TryFrom<&Element> for HeaderReference {
    type Error = Error;
    fn try_from(element: &Element) -> Result<Self, Self::Error> {
        element_to_entry(element)
    }
}

impl HeaderReference {
    fn into_inner(self) -> HeaderHash {
        self.0
    }
}

#[hdk_entry(id = "element_reference")]
struct ElementReference(HeaderHash);

impl TryFrom<&Element> for ElementReference {
    type Error = Error;
    fn try_from(element: &Element) -> Result<Self, Self::Error> {
        element_to_entry(element)
    }
}

impl ElementReference {
    fn into_inner(self) -> HeaderHash {
        self.0
    }
}

entry_defs![
    Something::entry_def(),
    EntryReference::entry_def(),
    HeaderReference::entry_def(),
    ElementReference::entry_def()
];

#[hdk_extern]
fn validate_create_entry_entry_reference(data: ValidateData) -> ExternResult<ValidateCallbackResult> {
    let entry_reference = EntryReference::try_from(&data.element)?;

    let _entry: EntryHashed = must_get_entry(entry_reference.into_inner())?;

    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn validate_create_entry_header_reference(data: ValidateData) -> ExternResult<ValidateCallbackResult> {
    let header_reference = HeaderReference::try_from(&data.element)?;

    let _header: SignedHeaderHashed = must_get_header(header_reference.into_inner())?;

    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn validate_create_entry_element_reference(data: ValidateData) -> ExternResult<ValidateCallbackResult> {
    let element_reference = ElementReference::try_from(&data.element)?;

    let _element: Element = must_get_valid_element(element_reference.into_inner())?;

    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn create_entry(_: ()) -> ExternResult<(HeaderHash, HeaderHash, HeaderHash, HeaderHash)> {
    let something = Something(vec![1, 2, 3]);
    let entry_hash: EntryHash = hdk::prelude::hash_entry(something.clone())?;
    let header_hash: HeaderHash = hdk::prelude::create_entry(something)?;

    // Commit some references to Something so we can test validation.
    let header_reference_hash = hdk::prelude::create_entry(HeaderReference(header_hash.clone()))?;
    let element_reference_hash = hdk::prelude::create_entry(ElementReference(header_hash.clone()))?;
    let entry_reference_hash = hdk::prelude::create_entry(EntryReference(entry_hash.clone()))?;

    Ok((header_hash, header_reference_hash, element_reference_hash, entry_reference_hash))
}

#[hdk_extern]
fn create_dangling_references(_: ()) -> ExternResult<(HeaderHash, HeaderHash, HeaderHash)> {
    let bad_header_hash = HeaderHash::from_raw_32(vec![0; 32]);
    let bad_entry_hash = EntryHash::from_raw_32(vec![0; 32]);

    Ok((
        hdk::prelude::create_entry(HeaderReference(bad_header_hash.clone()))?,
        hdk::prelude::create_entry(ElementReference(bad_header_hash))?,
        hdk::prelude::create_entry(EntryReference(bad_entry_hash))?,
    ))
}

#[hdk_extern]
fn must_get_valid_element(header_hash: HeaderHash) -> ExternResult<Element> {
    hdk::prelude::must_get_valid_element(header_hash)
}

#[hdk_extern]
fn must_get_header(header_hash: HeaderHash) -> ExternResult<SignedHeaderHashed> {
    hdk::prelude::must_get_header(header_hash)
}

#[hdk_extern]
fn must_get_entry(entry_hash: EntryHash) -> ExternResult<EntryHashed> {
    hdk::prelude::must_get_entry(entry_hash)
}