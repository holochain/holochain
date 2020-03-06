use super::chain_cas::ChainCasBuffer;
use sx_state::{
    buffer::{KvBuffer, KvvBuffer, StoreBuffer},
    db::DbManager,
    error::WorkspaceResult,
    prelude::{Reader, Writer},
};

mod genesis;
mod invoke_zome;
mod app_validation;
pub use genesis::GenesisWorkspace;
pub use invoke_zome::InvokeZomeWorkspace;
pub use app_validation::AppValidationWorkspace;

pub trait Workspace: Sized {
    fn commit_txn(self, writer: Writer) -> WorkspaceResult<()>;
}

#[cfg(test)]
pub mod tests {

    use super::{InvokeZomeWorkspace, Workspace};
    use sx_state::{
        buffer::{KvBuffer, StoreBuffer},
        db::{DbManager, CHAIN_ENTRIES, CHAIN_HEADERS},
        env::{ReadManager, WriteManager},
        error::WorkspaceResult,
        test_utils::test_env,
        prelude::{Reader, SingleStore, Writer},
    };
    use sx_types::prelude::*;
    use tempdir::TempDir;

    struct TestWorkspace<'env> {
        one: KvBuffer<'env, Address, u32>,
        two: KvBuffer<'env, Address, bool>,
    }

    impl<'env> TestWorkspace<'env> {
        pub fn new(
            reader: &'env Reader<'env>,
            dbs: &'env DbManager,
        ) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBuffer::new(reader, *dbs.get(&*CHAIN_ENTRIES)?)?,
                two: KvBuffer::new(reader, *dbs.get(&*CHAIN_HEADERS)?)?,
            })
        }
    }

    impl<'env> Workspace for TestWorkspace<'env> {
        fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
            self.one.flush_to_txn(&mut writer)?;
            self.two.flush_to_txn(&mut writer)?;
            writer.commit()?;
            Ok(())
        }
    }

    #[test]
    fn workspace_sanity_check() -> WorkspaceResult<()> {
        let env = test_env();
        let dbs = env.dbs()?;
        let addr1 = Address::from("hi".to_owned());
        let addr2 = Address::from("hi".to_owned());
        {
            let reader = env.reader()?;
            let mut workspace = TestWorkspace::new(&reader, &dbs)?;
            assert_eq!(workspace.one.get(&addr1)?, None);

            workspace.one.put(addr1.clone(), 1);
            workspace.two.put(addr2.clone(), true);
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
            assert_eq!(workspace.two.get(&addr2)?, Some(true));
            workspace.commit_txn(env.writer()?)?;
        }

        // Ensure that the data was persisted
        {
            let reader = env.reader()?;
            let workspace = TestWorkspace::new(&reader, &dbs)?;
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
        }
        Ok(())
    }
}
