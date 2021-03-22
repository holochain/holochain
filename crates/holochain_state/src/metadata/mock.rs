use super::*;

mock! {
    pub MetadataBuf
    {
        fn get_live_links<'a>(
            &self,
            key: &'a LinkMetaKey<'a>,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError>>>;
        fn get_links_all<'a>(
            &self,
            key: &'a LinkMetaKey<'a>,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError>>>;
        fn add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()>;
        fn delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()>;
        fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;
        fn register_rejected_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;
        fn register_element_header(&mut self, header: &Header) -> DatabaseResult<()>;
        fn register_rejected_element_header(&mut self, header: &Header) -> DatabaseResult<()>;
        fn register_activity(
            &mut self,
            header: &Header,
            validation_status: ValidationStatus,
        ) -> DatabaseResult<()>;
        fn register_activity_status(
            &mut self,
            agent: &AgentPubKey,
            status: ChainStatus,
        ) -> DatabaseResult<()>;
        fn register_activity_sequence(
            &mut self,
            agent: &AgentPubKey,
            sequence: Vec<(u32, HeaderHash)>,
            validation_status: ValidationStatus,
        ) -> DatabaseResult<()>;

        fn deregister_activity_sequence(&mut self, agent: &AgentPubKey, validation_status: ValidationStatus) -> DatabaseResult<()>;
        fn deregister_activity_status(&mut self, agent: &AgentPubKey) -> DatabaseResult<()>;
        fn register_activity_observed(
            &mut self,
            agent: &AgentPubKey,
            observed: HighestObserved,
        ) -> DatabaseResult<()>;
        fn deregister_activity_observed(&mut self, agent: &AgentPubKey) -> DatabaseResult<()>;
        fn register_update(&mut self, update: header::Update) -> DatabaseResult<()>;
        fn register_delete(&mut self, delete: header::Delete) -> DatabaseResult<()>;
        fn deregister_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;
        fn deregister_rejected_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;
        fn deregister_element_header(&mut self, header: HeaderHash) -> DatabaseResult<()>;
        fn deregister_rejected_element_header(&mut self, header: HeaderHash) -> DatabaseResult<()>;
        fn deregister_activity(
            &mut self,
            header: &Header,
            validation_status: ValidationStatus,
        ) -> DatabaseResult<()>;
        fn register_validation_package(
            &mut self,
            hash: &HeaderHash,
            package: Vec<HeaderHash>,
        );
        fn deregister_validation_package(&mut self, header: &HeaderHash);
        fn deregister_update(&mut self, update: header::Update) -> DatabaseResult<()>;
        fn deregister_delete(&mut self, delete: header::Delete) -> DatabaseResult<()>;
        fn register_raw_on_entry(&mut self, entry_hash: EntryHash, value: SysMetaVal) -> DatabaseResult<()>;
        fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal);
        fn register_validation_status(&mut self, hash: HeaderHash, status: ValidationStatus);
        fn deregister_validation_status(&mut self, hash: HeaderHash, status: ValidationStatus);
        fn deregister_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal);
        fn deregister_raw_on_entry(&mut self, entry_hash: EntryHash, value: SysMetaVal) -> DatabaseResult<()>;
        fn deregister_add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()>;
        fn deregister_delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()>;
        fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;
        fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;
        fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
        fn get_headers(
            &self,
            entry_hash: EntryHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn get_all_headers(
            &self,
            entry_hash: EntryHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn get_rejected_headers(
            &self,
            entry_hash: EntryHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn get_activity(
            &self,
            key: ChainItemKey,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn get_activity_sequence(
            &self,
            key: ChainItemKey,
        ) -> DatabaseResult<
            Box<dyn FallibleIterator<Item = (u32, HeaderHash), Error = DatabaseError>>,
        >;
        fn get_validation_package(
            &self,
            hash: &HeaderHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError>>>;
        fn get_activity_status(&self, agent: &AgentPubKey) -> DatabaseResult<Option<ChainStatus>>;
        fn get_activity_observed(&self, agent: &AgentPubKey)
        -> DatabaseResult<Option<HighestObserved>>;
        fn get_updates(
            &self,
            hash: AnyDhtHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn get_deletes_on_header(
            &self,
            new_entry_header: HeaderHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn get_deletes_on_entry(
            &self,
            entry_hash: EntryHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn get_validation_status(
            &self,
            hash: &HeaderHash,
        ) -> DatabaseResult<DisputedStatus>;
        fn get_link_removes_on_link_add(
            &self,
            link_add: HeaderHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn has_valid_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool>;
        fn has_any_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool>;
        fn has_rejected_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool>;
        fn has_registered_store_entry(&self, entry_hash: &EntryHash, header_hash: &HeaderHash) -> DatabaseResult<bool>;
        fn has_any_registered_store_entry(&self, hash: &EntryHash) -> DatabaseResult<bool>;
        fn env(&self) -> &EnvRead;
    }
}

impl MetadataBufT for MockMetadataBuf {
    fn get_live_links<'k, R: Readable>(
        &self,
        _r: &mut R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError>>> {
        MockMetadataBuf::get_live_links(&self, key)
    }

    fn get_links_all<'k, R: Readable>(
        &self,
        _r: &mut R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError>>> {
        MockMetadataBuf::get_links_all(&self, key)
    }

    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        self.get_canonical_entry_hash(entry_hash)
    }

    fn get_dht_status<R: Readable>(
        &self,
        _r: &mut R,
        entry_hash: &EntryHash,
    ) -> DatabaseResult<EntryDhtStatus> {
        MockMetadataBuf::get_dht_status(&self, entry_hash)
    }

    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        self.get_canonical_header_hash(header_hash)
    }

    fn get_headers<R: Readable>(
        &self,
        _reader: &mut R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>
    {
        self.get_headers(entry_hash)
    }

    fn get_rejected_headers<R: Readable>(
        &self,
        _reader: &mut R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>
    {
        self.get_rejected_headers(entry_hash)
    }

    fn get_all_headers<R: Readable>(
        &self,
        _reader: &mut R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>
    {
        self.get_all_headers(entry_hash)
    }

    fn get_activity<R: Readable>(
        &self,
        _reader: &mut R,
        key: ChainItemKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>
    {
        self.get_activity(key)
    }
    fn get_activity_sequence<R: Readable>(
        &self,
        _r: &mut R,
        key: ChainItemKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = (u32, HeaderHash), Error = DatabaseError>>>
    {
        self.get_activity_sequence(key)
    }

    fn get_validation_package<R: Readable>(
        &self,
        _r: &mut R,
        hash: &HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError>>> {
        self.get_validation_package(hash)
    }

    fn get_activity_status(&self, agent: &AgentPubKey) -> DatabaseResult<Option<ChainStatus>> {
        self.get_activity_status(agent)
    }

    fn get_activity_observed(
        &self,
        agent: &AgentPubKey,
    ) -> DatabaseResult<Option<HighestObserved>> {
        self.get_activity_observed(agent)
    }

    fn get_updates<R: Readable>(
        &self,
        _reader: &mut R,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>
    {
        self.get_updates(hash)
    }

    fn get_deletes_on_header<R: Readable>(
        &self,
        _reader: &mut R,
        new_entry_header: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>
    {
        self.get_deletes_on_header(new_entry_header)
    }

    fn get_deletes_on_entry<R: Readable>(
        &self,
        _reader: &mut R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>
    {
        self.get_deletes_on_entry(entry_hash)
    }

    fn get_link_removes_on_link_add<R: Readable>(
        &self,
        _reader: &mut R,
        link_add: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>
    {
        self.get_link_removes_on_link_add(link_add)
    }
    fn get_validation_status<R: Readable>(
        &self,
        _r: &mut R,
        hash: &HeaderHash,
    ) -> DatabaseResult<DisputedStatus> {
        self.get_validation_status(hash)
    }

    fn add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()> {
        self.add_link(link_add)
    }

    fn delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()> {
        self.delete_link(link_remove)
    }

    fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        self.register_header(new_entry_header)
    }

    fn register_rejected_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        self.register_rejected_header(new_entry_header)
    }

    fn deregister_rejected_header(
        &mut self,
        new_entry_header: NewEntryHeader,
    ) -> DatabaseResult<()> {
        self.deregister_rejected_header(new_entry_header)
    }

    fn register_element_header(&mut self, header: &Header) -> DatabaseResult<()> {
        self.register_element_header(header)
    }

    fn register_rejected_element_header(&mut self, header: &Header) -> DatabaseResult<()> {
        self.register_rejected_element_header(header)
    }

    fn deregister_rejected_element_header(&mut self, hash: HeaderHash) -> DatabaseResult<()> {
        self.deregister_rejected_element_header(hash)
    }

    fn register_activity(
        &mut self,
        header: &Header,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()> {
        self.register_activity(header, validation_status)
    }
    /// Register a sequence of activity onto an agent key
    fn register_activity_sequence(
        &mut self,
        agent: &AgentPubKey,
        sequence: impl IntoIterator<Item = (u32, HeaderHash)>,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()> {
        self.register_activity_sequence(agent, sequence.into_iter().collect(), validation_status)
    }

    /// Deregister a sequence of activity onto an agent key
    fn deregister_activity_sequence(
        &mut self,
        agent: &AgentPubKey,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()> {
        self.deregister_activity_sequence(agent, validation_status)
    }
    fn register_activity_status(
        &mut self,
        agent: &AgentPubKey,
        status: ChainStatus,
    ) -> DatabaseResult<()> {
        self.register_activity_status(agent, status)
    }
    fn deregister_activity_status(&mut self, agent: &AgentPubKey) -> DatabaseResult<()> {
        self.deregister_activity_status(agent)
    }
    fn register_activity_observed(
        &mut self,
        agent: &AgentPubKey,
        observed: HighestObserved,
    ) -> DatabaseResult<()> {
        self.register_activity_observed(agent, observed)
    }
    fn deregister_activity_observed(&mut self, agent: &AgentPubKey) -> DatabaseResult<()> {
        self.deregister_activity_observed(agent)
    }

    fn register_update(&mut self, update: header::Update) -> DatabaseResult<()> {
        self.register_update(update)
    }

    fn register_delete(&mut self, delete: header::Delete) -> DatabaseResult<()> {
        self.register_delete(delete)
    }

    fn deregister_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        self.deregister_header(new_entry_header)
    }
    fn deregister_element_header(&mut self, header: HeaderHash) -> DatabaseResult<()> {
        self.deregister_element_header(header)
    }

    fn deregister_activity(
        &mut self,
        header: &Header,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()> {
        self.deregister_activity(header, validation_status)
    }

    fn register_validation_package(
        &mut self,
        hash: &HeaderHash,
        package: impl IntoIterator<Item = HeaderHash>,
    ) {
        self.register_validation_package(hash, package.into_iter().collect())
    }

    fn deregister_validation_package(&mut self, header: &HeaderHash) {
        self.deregister_validation_package(header)
    }

    fn deregister_update(&mut self, update: header::Update) -> DatabaseResult<()> {
        self.deregister_update(update)
    }

    fn deregister_delete(&mut self, delete: header::Delete) -> DatabaseResult<()> {
        self.deregister_delete(delete)
    }

    fn deregister_add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()> {
        self.deregister_add_link(link_add)
    }

    /// Deregister a remove link
    fn deregister_delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()> {
        self.deregister_delete_link(link_remove)
    }

    fn register_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()> {
        self.register_raw_on_entry(entry_hash, value)
    }

    fn deregister_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()> {
        self.deregister_raw_on_entry(entry_hash, value)
    }

    fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal) {
        self.register_raw_on_header(header_hash, value)
    }
    fn deregister_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal) {
        self.deregister_raw_on_header(header_hash, value)
    }
    fn register_validation_status(&mut self, hash: HeaderHash, status: ValidationStatus) {
        self.register_validation_status(hash, status)
    }
    fn deregister_validation_status(&mut self, hash: HeaderHash, status: ValidationStatus) {
        self.deregister_validation_status(hash, status)
    }
    fn has_any_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool> {
        self.has_any_registered_store_element(hash)
    }
    fn has_valid_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool> {
        self.has_valid_registered_store_element(hash)
    }
    fn has_rejected_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool> {
        self.has_rejected_registered_store_element(hash)
    }
    fn has_registered_store_entry(
        &self,
        entry_hash: &EntryHash,
        header_hash: &HeaderHash,
    ) -> DatabaseResult<bool> {
        self.has_registered_store_entry(entry_hash, header_hash)
    }
    fn has_any_registered_store_entry(&self, hash: &EntryHash) -> DatabaseResult<bool> {
        self.has_any_registered_store_entry(hash)
    }

    fn env(&self) -> &EnvRead {
        self.env()
    }
}
