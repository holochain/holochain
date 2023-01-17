use crate::conductor::interface::SignalBroadcaster;
use crate::conductor::ConductorHandle;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDna;
use holochain_serialized_bytes::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_types::prelude::*;

pub const POST_COMMIT_CHANNEL_BOUND: usize = 100;
pub const POST_COMMIT_CONCURRENT_LIMIT: usize = 5;

#[derive(Clone)]
pub struct PostCommitInvocation {
    zome: CoordinatorZome,
    actions: Vec<SignedActionHashed>,
}

impl PostCommitInvocation {
    pub fn new(zome: CoordinatorZome, actions: Vec<SignedActionHashed>) -> Self {
        Self { zome, actions }
    }
}

#[derive(Clone, Constructor)]
pub struct PostCommitHostAccess {
    pub workspace: HostFnWorkspace,
    pub keystore: MetaLairClient,
    pub network: HolochainP2pDna,
    pub signal_tx: SignalBroadcaster,
}

impl std::fmt::Debug for PostCommitHostAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostCommitHostAccess").finish()
    }
}

impl From<PostCommitHostAccess> for HostContext {
    fn from(post_commit_host_access: PostCommitHostAccess) -> Self {
        Self::PostCommit(post_commit_host_access)
    }
}

impl From<&PostCommitHostAccess> for HostFnAccess {
    fn from(_: &PostCommitHostAccess) -> Self {
        let mut access = Self::all();
        // Post commit happens after all workspace writes are complete.
        // Writing more to the workspace becomes circular.
        // If you need to trigger some more writes, try a `call_remote` back
        // into the current cell.
        access.write_workspace = Permission::Deny;
        access
    }
}

impl Invocation for PostCommitInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::OneCoordinator(self.zome.to_owned())
    }
    fn fn_components(&self) -> FnComponents {
        vec!["post_commit".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(self.actions)
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

impl TryFrom<PostCommitInvocation> for ExternIO {
    type Error = SerializedBytesError;
    fn try_from(post_commit_invocation: PostCommitInvocation) -> Result<Self, Self::Error> {
        ExternIO::encode(&post_commit_invocation.actions)
    }
}

pub async fn send_post_commit(
    conductor_handle: ConductorHandle,
    workspace: SourceChainWorkspace,
    network: HolochainP2pDna,
    keystore: MetaLairClient,
    actions: Vec<SignedActionHashed>,
    zomes: Vec<CoordinatorZome>,
) -> Result<(), tokio::sync::mpsc::error::SendError<()>> {
    let cell_id = workspace.source_chain().cell_id();

    for zome in zomes {
        conductor_handle
            .post_commit_permit()
            .await?
            .send(PostCommitArgs {
                host_access: PostCommitHostAccess {
                    workspace: workspace.clone().into(),
                    keystore: keystore.clone(),
                    network: network.clone(),
                    signal_tx: conductor_handle.signal_broadcaster(),
                },
                invocation: PostCommitInvocation::new(zome, actions.clone()),
                cell_id: cell_id.clone(),
            });
    }
    Ok(())
}

#[derive(Clone)]
pub struct PostCommitArgs {
    pub host_access: PostCommitHostAccess,
    pub invocation: PostCommitInvocation,
    pub cell_id: CellId,
}

#[cfg(test)]
mod test {
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::PostCommitHostAccessFixturator;
    use crate::fixt::PostCommitInvocationFixturator;
    use holochain_types::prelude::*;
    use holochain_zome_types::ExternIO;

    #[tokio::test(flavor = "multi_thread")]
    async fn post_commit_invocation_access() {
        let post_commit_host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let mut expected = HostFnAccess::all();
        expected.write_workspace = Permission::Deny;
        assert_eq!(HostFnAccess::from(&post_commit_host_access), expected);
    }

    #[test]
    fn post_commit_invocation_zomes() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let zome = post_commit_invocation.zome.clone();
        assert_eq!(
            ZomesToInvoke::OneCoordinator(zome),
            post_commit_invocation.zomes(),
        );
    }

    #[test]
    fn post_commit_invocation_fn_components() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();

        let mut expected = vec!["post_commit"];
        for fn_component in post_commit_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[test]
    fn post_commit_invocation_host_input() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();

        let host_input = post_commit_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            ExternIO::encode(ActionHashVecFixturator::new(::fixt::Empty).next().unwrap()).unwrap(),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::PostCommitHostAccessFixturator;
    use crate::fixt::PostCommitInvocationFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_post_commit_unimplemented() {
        let host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome = CoordinatorZome::from(TestWasm::Foo);

        let result = ribosome
            .run_post_commit(host_access, post_commit_invocation)
            .unwrap();
        assert_eq!(result, ());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_post_commit_implemented_success() {
        let host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::PostCommitSuccess]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome = CoordinatorZome::from(TestWasm::PostCommitSuccess);

        let result = ribosome
            .run_post_commit(host_access, post_commit_invocation)
            .unwrap();
        assert_eq!(result, ());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "flakey. Sometimes fails the second last assert with 3 instead of 5"]
    #[cfg(feature = "test_utils")]
    async fn post_commit_test_volley() -> anyhow::Result<()> {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            bob,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::PostCommitVolley).await;

        let _set_access: () = conductor.call::<_, (), _>(&alice, "set_access", ()).await;

        let _set_access: () = conductor.call::<_, (), _>(&bob, "set_access", ()).await;

        let _ping: ActionHash = conductor.call(&alice, "ping", bob_pubkey).await;

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let alice_query: Vec<Record> = conductor.call(&alice, "query", ()).await;

        assert_eq!(alice_query.len(), 5);

        let bob_query: Vec<Record> = conductor.call(&bob, "query", ()).await;

        assert_eq!(bob_query.len(), 4);

        Ok(())
    }
}
