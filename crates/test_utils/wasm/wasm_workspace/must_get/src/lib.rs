use hdk::prelude::*;

#[hdk_entry(id = "something")]
#[derive(Clone)]
struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

#[hdk_entry(id = "entry_reference")]
struct EntryReference(EntryHash);

impl EntryReference {
    fn into_inner(self) -> EntryHash {
        self.0
    }
}

#[hdk_entry(id = "header_reference")]
struct HeaderReference(HeaderHash);

impl HeaderReference {
    fn into_inner(self) -> HeaderHash {
        self.0
    }
}

#[hdk_entry(id = "element_reference")]
struct ElementReference(HeaderHash);

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
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    let this_zome = zome_info()?;

    if let Op::StoreEntry {
        header:
            SignedHashed {
                header:
                    EntryCreationHeader::Create(Create { entry_type, .. })
                    | EntryCreationHeader::Update(Update { entry_type, .. }),
                ..
            },
        entry,
    } = op
    {
        if let EntryType::App(app_entry_type) = entry_type {
            if this_zome.matches_entry_def_id(&app_entry_type, EntryReference::entry_def_id()) {
                let entry_reference = EntryReference::try_from(&entry)?;
                let entry_hashed: EntryHashed = must_get_entry(entry_reference.into_inner())?;
                let entry: Entry = entry_hashed.clone().into();
                let _something_hashed: Something = entry_hashed.try_into()?;
                let _something: Something = entry.try_into()?;
            }
            if this_zome.matches_entry_def_id(&app_entry_type, HeaderReference::entry_def_id()) {
                let header_reference = HeaderReference::try_from(&entry)?;
                let signed_header_hashed: SignedHeaderHashed =
                    must_get_header(header_reference.into_inner())?;
                let _header: Header = signed_header_hashed.into();
            }
            if this_zome.matches_entry_def_id(&app_entry_type, ElementReference::entry_def_id()) {
                let element_reference = ElementReference::try_from(&entry)?;

                let element: Element = must_get_valid_element(element_reference.into_inner())?;
                let _something: Something = element.try_into()?;
            }
        }
    }

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
    let entry_reference_hash = hdk::prelude::create_entry(EntryReference(entry_hash))?;

    Ok((
        header_hash,
        header_reference_hash,
        element_reference_hash,
        entry_reference_hash,
    ))
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
