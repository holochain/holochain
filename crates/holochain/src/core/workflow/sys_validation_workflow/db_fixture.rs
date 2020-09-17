use super::*;
use crate::core::{
    state::element_buf::ElementBufFixtureItem as EF, state::metadata::MetadataBufFixtureItem as MF,
    state::metadata::SysMetaKey, state::metadata::SysMetaVal,
    workflow::sys_validation_workflow::SysValidationWorkspaceFixtureItem as SF,
};
use holochain_state::{db_fixture::DbFixture, db_fixture::LoadDbFixture};
use holochain_types::EntryHashed;
use maplit::btreeset;

// TODO: could DRY these predictable impls with proc macros
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SysValidationWorkspaceFixtureItem {
    IntegrationLimbo(<IntegrationLimboStore as LoadDbFixture>::FixtureItem),
    ValidationLimbo(<ValidationLimboStore as LoadDbFixture>::FixtureItem),
    ElementVault(<ElementBuf as LoadDbFixture>::FixtureItem),
    MetaVault(<MetadataBuf as LoadDbFixture>::FixtureItem),
    ElementPending(<ElementBuf<PendingPrefix> as LoadDbFixture>::FixtureItem),
    MetaPending(<MetadataBuf<PendingPrefix> as LoadDbFixture>::FixtureItem),
    ElementJudged(<ElementBuf<JudgedPrefix> as LoadDbFixture>::FixtureItem),
    MetaJudged(<MetadataBuf<JudgedPrefix> as LoadDbFixture>::FixtureItem),
    ElementCache(<ElementBuf as LoadDbFixture>::FixtureItem),
    MetaCache(<MetadataBuf as LoadDbFixture>::FixtureItem),
}

impl SysValidationWorkspaceFixtureItem {
    pub fn registered_entry(el: Element) -> DbFixture<SysValidationWorkspace> {
        let (shh, maybe_entry) = el.into_inner();
        let header = shh.header_hashed().clone().into();
        let entry = EntryHashed::from_content_sync(maybe_entry.into_option().unwrap());
        let entry_hash = entry.as_hash().clone();
        btreeset! {
            SF::ElementVault(EF::PublicEntries(entry)),
            SF::ElementVault(EF::Headers(shh.into())),
            SF::MetaCache(MF::SystemMeta((SysMetaKey::from(entry_hash).into(), SysMetaVal::NewEntry(header))))
        }
    }
}

impl LoadDbFixture for SysValidationWorkspace {
    type FixtureItem = SysValidationWorkspaceFixtureItem;

    fn write_test_datum(&mut self, datum: Self::FixtureItem) {
        match datum {
            Self::FixtureItem::IntegrationLimbo(d) => self.integration_limbo.write_test_datum(d),
            Self::FixtureItem::ValidationLimbo(d) => self.validation_limbo.write_test_datum(d),
            Self::FixtureItem::ElementVault(d) => self.element_vault.write_test_datum(d),
            Self::FixtureItem::MetaVault(d) => self.meta_vault.write_test_datum(d),
            Self::FixtureItem::ElementPending(d) => self.element_pending.write_test_datum(d),
            Self::FixtureItem::MetaPending(d) => self.meta_pending.write_test_datum(d),
            Self::FixtureItem::ElementJudged(d) => self.element_judged.write_test_datum(d),
            Self::FixtureItem::MetaJudged(d) => self.meta_judged.write_test_datum(d),
            Self::FixtureItem::ElementCache(d) => self.element_cache.write_test_datum(d),
            Self::FixtureItem::MetaCache(d) => self.meta_cache.write_test_datum(d),
        }
    }

    fn read_test_data<R: Readable>(&self, reader: &R) -> DbFixture<Self> {
        let integration_limbo = self
            .integration_limbo
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::IntegrationLimbo(i));

        let validation_limbo = self
            .validation_limbo
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::ValidationLimbo(i));

        let element_vault = self
            .element_vault
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::ElementVault(i));

        let meta_vault = self
            .meta_vault
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::MetaVault(i));

        let element_pending = self
            .element_pending
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::ElementPending(i));

        let meta_pending = self
            .meta_pending
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::MetaPending(i));

        let element_judged = self
            .element_judged
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::ElementJudged(i));

        let meta_judged = self
            .meta_judged
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::MetaJudged(i));

        let element_cache = self
            .element_cache
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::ElementCache(i));

        let meta_cache = self
            .meta_cache
            .read_test_data(reader)
            .into_iter()
            .map(|i| Self::FixtureItem::MetaCache(i));

        integration_limbo
            .chain(validation_limbo)
            .chain(element_vault)
            .chain(meta_vault)
            .chain(element_pending)
            .chain(meta_pending)
            .chain(element_judged)
            .chain(meta_judged)
            .chain(element_cache)
            .chain(meta_cache)
            .collect::<DbFixture<Self>>()
    }
}
