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
        fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()>;
        fn remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()>;
        fn sync_add_create(&self, create: header::EntryCreate) -> DatabaseResult<()>;
        fn sync_register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;
        fn sync_register_activity(
            &mut self,
            header: Header,
        ) -> DatabaseResult<()>;
        fn sync_register_update(&mut self, update: header::EntryUpdate) -> DatabaseResult<()>;
        fn sync_register_delete(&mut self, delete: header::ElementDelete) -> DatabaseResult<()>;
        fn register_raw_on_entry(&mut self, entry_hash: EntryHash, value: SysMetaVal) -> DatabaseResult<()>;
        fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal);
        fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;
        fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;
        fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
        fn get_headers(
            &self,
            entry_hash: EntryHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
        fn get_activity(
            &self,
            header_hash: AgentPubKey,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError>>>;
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
        agent_pubkey: AgentPubKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        self.get_activity(agent_pubkey)
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

    async fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()> {
        self.add_link(link_add)
    }

    async fn remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()> {
        self.remove_link(link_remove)
    }

    async fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        self.sync_register_header(new_entry_header)
    }

    async fn register_activity(&mut self, header: Header) -> DatabaseResult<()> {
        self.sync_register_activity(header)
    }

    async fn register_update(&mut self, update: header::EntryUpdate) -> DatabaseResult<()> {
        self.sync_register_update(update)
    }

    async fn register_delete(&mut self, delete: header::ElementDelete) -> DatabaseResult<()> {
        self.sync_register_delete(delete)
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
}
