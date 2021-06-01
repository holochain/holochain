
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

    let (_element, _valid): (Element, bool) = must_get_element(element_reference.into_inner())?;

    Ok(ValidateCallbackResult::Valid)
}