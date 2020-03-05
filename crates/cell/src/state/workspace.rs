use super::chain_cas::ChainCasBuffer;
use sx_state::{
    buffer::{KvBuffer, KvvBuffer, StoreBuffer},
    db::DbManager,
    error::WorkspaceResult,
    Reader, Writer,
};

pub trait Workspace<'txn>: Sized {
    fn finalize(self, writer: Writer) -> WorkspaceResult<()>;
}

pub struct InvokeZomeWorkspace<'env> {
    cas: ChainCasBuffer<'env>,
    // meta: KvvBuffer<'env, String, String>,
}

// impl<'env> Workspace<'env> for InvokeZomeWorkspace<'env> {
//     fn finalize(self, mut writer: Writer) -> WorkspaceResult<()> {
//         self.cas.finalize(&mut writer)?;
//         // self.meta.finalize(&mut writer)?;
//         writer.commit()?;
//         Ok(())
//     }
// }

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(reader: &'env Reader<'env>, dbm: &'env DbManager<'env>) -> WorkspaceResult<Self> {
        Ok(Self {
            cas: ChainCasBuffer::primary(reader, dbm)?,
            // meta: KvvBuffer::new(reader, dbm)?,
        })
    }

    pub fn cas(&mut self) -> &mut ChainCasBuffer<'env> {
        &mut self.cas
    }
}

pub struct AppValidationWorkspace;

impl<'env> Workspace<'env> for AppValidationWorkspace {
    fn finalize(self, _writer: Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod tests {

    use super::{InvokeZomeWorkspace, Workspace};
    use sx_state::{
        buffer::{KvBuffer, StoreBuffer},
        db::{DbManager, CHAIN_ENTRIES, CHAIN_HEADERS},
        env::{ReadManager, WriteManager},
        error::WorkspaceResult,
        Reader, SingleStore, Writer, test_utils::test_env,
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
            dbm: &'env DbManager<'env>,
        ) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBuffer::new(reader, *dbm.get(&*CHAIN_ENTRIES)?)?,
                two: KvBuffer::new(reader, *dbm.get(&*CHAIN_HEADERS)?)?,
            })
        }
    }

    impl<'env> Workspace<'env> for TestWorkspace<'env> {
        fn finalize(self, mut writer: Writer) -> WorkspaceResult<()> {
            self.one.finalize(&mut writer)?;
            self.two.finalize(&mut writer)?;
            writer.commit()?;
            Ok(())
        }
    }

    #[test]
    fn workspace_sanity_check() {
        let arc = test_env();
        let env = arc.env();
        let dbm = arc.dbs().unwrap();
        let addr1 = Address::from("hi".to_owned());
        let addr2 = Address::from("hi".to_owned());
        {
            let reader = env.reader().unwrap();
            let mut workspace = TestWorkspace::new(&reader, &dbm).unwrap();
            assert_eq!(workspace.one.get(&addr1).unwrap(), None);

            workspace.one.put(addr1.clone(), 1);
            workspace.two.put(addr2.clone(), true);
            assert_eq!(workspace.one.get(&addr1).unwrap(), Some(1));
            assert_eq!(workspace.two.get(&addr2).unwrap(), Some(true));
            workspace.finalize(env.writer().unwrap()).unwrap();
        }

        // Ensure that the data was persisted
        {
            let reader = env.reader().unwrap();
            let workspace = TestWorkspace::new(&reader, &dbm).unwrap();
            assert_eq!(workspace.one.get(&addr1).unwrap(), Some(1));
        }
    }
}
