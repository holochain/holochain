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
        fn sync_register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;
        fn sync_register_element_header(&mut self, header: &Header) -> DatabaseResult<()>;
        fn sync_register_activity(
            &mut self,
            header: &Header,
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
        ) -> DatabaseResult<()>;

        fn deregister_activity_sequence(&mut self, agent: &AgentPubKey) -> DatabaseResult<()>;
        fn deregister_activity_status(&mut self, agent: &AgentPubKey) -> DatabaseResult<()>;
        fn register_activity_observed(
            &mut self,
            agent: &AgentPubKey,
            observed: HighestObserved,
        ) -> DatabaseResult<()>;
        fn deregister_activity_observed(&mut self, agent: &AgentPubKey) -> DatabaseResult<()>;
        fn sync_register_update(&mut self, update: header::Update) -> DatabaseResult<()>;
        fn sync_register_delete(&mut self, delete: header::Delete) -> DatabaseResult<()>;
        fn sync_deregister_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;
        fn sync_deregister_element_header(&mut self, header: HeaderHash) -> DatabaseResult<()>;
        fn sync_deregister_activity(
            &mut self,
            header: &Header,
        ) -> DatabaseResult<()>;
        fn register_validation_package(
            &mut self,
            hash: &HeaderHash,
            package: Vec<HeaderHash>,
        );
        fn deregister_validation_package(&mut self, header: &HeaderHash);
        fn sync_deregister_update(&mut self, update: header::Update) -> DatabaseResult<()>;
        fn sync_deregister_delete(&mut self, delete: header::Delete) -> DatabaseResult<()>;
        fn register_raw_on_entry(&mut self, entry_hash: EntryHash, value: SysMetaVal) -> DatabaseResult<()>;
        fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal);
        fn sync_deregister_add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()>;
        fn sync_deregister_delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()>;
        fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;
        fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;
        fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
        fn get_headers(
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
        fn get_link_removes_on_link_add(
            &self,
            link_add: HeaderHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn has_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool>;
        fn has_registered_store_entry(&self, entry_hash: &EntryHash, header_hash: &HeaderHash) -> DatabaseResult<bool>;
        fn has_any_registered_store_entry(&self, hash: &EntryHash) -> DatabaseResult<bool>;
        fn env(&self) -> &EnvironmentRead;
    }
}

#[async_trait::async_trait]
impl MetadataBufT for MockMetadataBuf {
    fn get_live_links<'r, 'k, R: Readable>(
        &'r self,
        _r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>
    {
        MockMetadataBuf::get_live_links(&self, key)
    }

    fn get_links_all<'r, 'k, R: Readable>(
        &'r self,
        _r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>
    {
        MockMetadataBuf::get_links_all(&self, key)
    }

    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        self.get_canonical_entry_hash(entry_hash)
    }

    fn get_dht_status<'r, R: Readable>(
        &'r self,
        _r: &'r R,
        entry_hash: &EntryHash,
    ) -> DatabaseResult<EntryDhtStatus> {
        MockMetadataBuf::get_dht_status(&self, entry_hash)
    }

    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        self.get_canonical_header_hash(header_hash)
    }

    fn get_headers<'r, R: Readable>(
        &'r self,
        _reader: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        self.get_headers(entry_hash)
    }

    fn get_activity<'r, R: Readable>(
        &'r self,
        _reader: &'r R,
        key: ChainItemKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        self.get_activity(key)
    }
    fn get_activity_sequence<'r, R: Readable>(
        &'r self,
        _r: &'r R,
        key: ChainItemKey,
    ) -> DatabaseResult<
        Box<dyn FallibleIterator<Item = (u32, HeaderHash), Error = DatabaseError> + '_>,
    > {
        self.get_activity_sequence(key)
    }

    fn get_validation_package<'r, R: Readable>(
        &'r self,
        _r: &'r R,
        hash: &HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
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

    fn get_updates<'r, R: Readable>(
        &'r self,
        _reader: &'r R,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        self.get_updates(hash)
    }

    fn get_deletes_on_header<'r, R: Readable>(
        &'r self,
        _reader: &'r R,
        new_entry_header: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        self.get_deletes_on_header(new_entry_header)
    }

    fn get_deletes_on_entry<'r, R: Readable>(
        &'r self,
        _reader: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        self.get_deletes_on_entry(entry_hash)
    }

    fn get_link_removes_on_link_add<'r, R: Readable>(
        &'r self,
        _reader: &'r R,
        link_add: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        self.get_link_removes_on_link_add(link_add)
    }

    fn add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()> {
        self.add_link(link_add)
    }

    fn delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()> {
        self.delete_link(link_remove)
    }

    fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        self.sync_register_header(new_entry_header)
    }
    fn register_element_header(&mut self, header: &Header) -> DatabaseResult<()> {
        self.sync_register_element_header(header)
    }

    fn register_activity(&mut self, header: &Header) -> DatabaseResult<()> {
        self.sync_register_activity(header)
    }
    /// Register a sequence of activity onto an agent key
    fn register_activity_sequence(
        &mut self,
        agent: &AgentPubKey,
        sequence: impl IntoIterator<Item = (u32, HeaderHash)>,
    ) -> DatabaseResult<()> {
        self.register_activity_sequence(agent, sequence.into_iter().collect())
    }

    /// Deregister a sequence of activity onto an agent key
    fn deregister_activity_sequence(&mut self, agent: &AgentPubKey) -> DatabaseResult<()> {
        self.deregister_activity_sequence(agent)
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
        self.sync_register_update(update)
    }

    fn register_delete(&mut self, delete: header::Delete) -> DatabaseResult<()> {
        self.sync_register_delete(delete)
    }

    fn deregister_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        self.sync_deregister_header(new_entry_header)
    }
    fn deregister_element_header(&mut self, header: HeaderHash) -> DatabaseResult<()> {
        self.sync_deregister_element_header(header)
    }

    fn deregister_activity(&mut self, header: &Header) -> DatabaseResult<()> {
        self.sync_deregister_activity(header)
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
        self.sync_deregister_update(update)
    }

    fn deregister_delete(&mut self, delete: header::Delete) -> DatabaseResult<()> {
        self.sync_deregister_delete(delete)
    }

    fn deregister_add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()> {
        self.sync_deregister_add_link(link_add)
    }

    /// Deregister a remove link
    fn deregister_delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()> {
        self.sync_deregister_delete_link(link_remove)
    }

    fn register_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()> {
        self.register_raw_on_entry(entry_hash, value)
    }

    fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal) {
        self.register_raw_on_header(header_hash, value)
    }
    fn has_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool> {
        self.has_registered_store_element(hash)
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

    fn env(&self) -> &EnvironmentRead {
        self.env()
    }
}
