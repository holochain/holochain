struct Header(HeaderContent, Signature);

struct DhtOp {
    header: Header,
    data: OpData,
}

enum OpData {
    #[cfg(feature = "strict")]
    StoreHeader {
        entry: EntryContent,
    },
    #[cfg(not(feature = "strict"))]
    StoreHeader,
    StoreEntry {
        entry: EntryContent,
    },
    RegisterAgentActivity,
    RegisterUpdatedTo {
        entry: EntryContent,
    },
    RegisterDeletedBy {
        entry: DeleteEntry,
    },
    RegisterAddLink {
        entry: AddLinkEntry,
    },
    RegisterRemoveLink {
        entry: RemoveLinkEntry,
    },
}

impl DhtOp {
    fn neighborhood(self) -> Address {
        use OpData::*;

        let DhtOp {
            header: Header(header_content, ..),
            data,
        } = op;
        match data {
            StoreHeader { .. } => hash(header_content),
            StoreEntry { .. } => header.entry_hash,
            RegisterAgentActivity => header.author_key,
            RegisterUpdatedTo { .. } => header.replaces,
            RegisterDeletedBy { entry } => entry.deletes,
            RegisterAddLink { entry } => entry.base,
            RegisterRemoveLink { entry } => entry.base,
        }
    }

    fn from(commit: A) {
        // ... construct DHT Op Transform
    }
}

#[derive(Hash)]
enum HashReadyForm<'a> {
    // optimization: don't hash signature. it is redundant with header and therefore wastes hash time to include
    StoredHeader {
        header_content: &'a HeaderContent,
    },
    StoredEntry {
        header_content: &'a HeaderContent,
    },
    RegisteredAgentActivity {
        header_content: &'a HeaderContent,
    },
    RegisteredUpdatedTo {
        entry: &'a EntryContent,
        replaces: &'a Address,
    },
    RegisteredDeletedBy {
        entry: &'a DeleteEntry,
    },
    RegisteredAddLink {
        header_content: &'a HeaderContent,
    },
    // ^ future work: encode idempotency in LinkAdd entries themselves
    RegisteredRemoveLink {
        header_content: &'a HeaderContent,
    },
    // ^ if LinkAdds were idempotent then this could just be entry.
}

fn unique_hash(op: &DhtOp) -> HashReadyForm<'_> {
    use OpData::*;

    let DhtOp {
        header: Header(header_content, ..),
        data,
    } = op;
    match data {
        StoreHeader { .. } => HashReadyForm::StoredHeader { header_content },
        StoreEntry { .. } => HashReadyForm::StoredEntry { header_content },
        RegisterAgentActivity => HashReadyForm::RegisteredAgentActivity { header_content },
        RegisterUpdatedTo { entry } => HashReadyForm::RegisteredUpdatedTo {
            entry,
            replaces: header.replaces(),
        },
        RegisterDeletedBy { entry } => HashReadyForm::RegisteredDeletedBy { entry },
        RegisterAddLink { .. } => HashReadyForm::RegisteredAddLink { header_content },
        RegisterRemoveLink { .. } => HashReadyForm::RegisteredRemoveLink { header_content },
    }
}

fn ops_from_commit(header: ChainHeader, entry: Entry, callback: impl FnMut(&DhtOp)) {
    // FIXME: avoid cloning headers and entries

    callback(DhtOp {
        header: header.clone(),
        data: if cfg!(feature = "strict") {
            OpData::StoreHeader {
                entry: entry.clone(),
            }
        } else {
            OpData::StoreHeader
        },
    });
    callback(DhtOp {
        header: header.clone(),
        data: OpData::StoreEntry {
            entry: entry.clone(),
        },
    });
    callback(DhtOp {
        header: header.clone(),
        data: OpData::RegisterAgentActivity,
    });

    match entry {
        Entry::DeleteEntry(entry) => {
            callback(DhtOp {
                header,
                data: OpData::RegisterDeletedBy { entry },
            });
        }
        Entry::LinkAdd(entry) => {
            callback(DhtOp {
                header,
                data: OpData::RegisterAddLink { entry },
            });
        }
        Entry::LinkRemove(entry) => callback(DhtOp {
            header,
            data: OpData::RegisterRemoveLink { entry },
        }),
        entry if header.link_crud.is_some() => callback(DhtOp {
            header,
            data: OpData::RegisterUpdatedTo { entry },
        }),
    }
}
