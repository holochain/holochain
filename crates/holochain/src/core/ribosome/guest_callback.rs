pub mod entry_defs;
pub mod genesis_self_check;
pub mod init;
pub mod post_commit;
pub mod validate;
use super::{HostContext, Ribosome};
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::Invocation;
use holochain_types::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::task::JoinHandle;

pub type CallStreamItem = Result<(Zome, ExternIO), (Zome, RibosomeError)>;
pub type CallStream = tokio_stream::wrappers::ReceiverStream<CallStreamItem>;

pub fn call_stream(
    host_context: HostContext,
    ribosome: Ribosome,
    invocation: Arc<dyn Invocation + 'static>,
) -> (
    CallStream,
    JoinHandle<Result<(), tokio::sync::mpsc::error::SendError<CallStreamItem>>>,
) {
    let (tx, rx) = tokio::sync::mpsc::channel(1);

    let h = tokio::spawn(async move {
        let mut remaining_zomes: VecDeque<_> = ribosome
            .zomes_to_invoke(invocation.zomes())
            .into_iter()
            .collect();
        let remaining_components_original: VecDeque<_> = invocation.fn_components().collect();

        while let Some(zome) = remaining_zomes.pop_front() {
            // reset fn components
            let mut remaining_components = remaining_components_original.clone();
            while let Some(to_call) = remaining_components.pop_front() {
                let to_call = to_call.into();
                let r = ribosome
                    .maybe_call(host_context.clone(), invocation.clone(), &zome, &to_call)
                    .await;
                match r {
                    Ok(None) => {}
                    Ok(Some(result)) => tx.send(Ok((zome.clone(), result))).await?,
                    Err(e) => tx.send(Err((zome.clone(), e))).await?,
                }
            }
        }
        Ok(())
    });
    (tokio_stream::wrappers::ReceiverStream::new(rx), h)
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod tests {
    use crate::core::ribosome::guest_callback::call_stream;
    use crate::core::ribosome::mock_ribosome::MockRibosomeBuilder;
    use crate::core::ribosome::MockInvocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::core::ribosome::{FnComponents, InvocationAuth};
    use crate::fixt::FnComponentsFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use crate::fixt::ZomeFixturator;
    use holochain_types::prelude::*;
    use mockall::predicate::*;
    use mockall::Sequence;
    use tokio_stream::StreamExt;

    #[tokio::test(flavor = "multi_thread")]
    async fn call_stream_streams() {
        let mut sequence = Sequence::new();

        let mut invocation = MockInvocation::new();

        let host_access = ZomeCallHostAccessFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        let zome_fixturator = ZomeFixturator::new(::fixt::Unpredictable);
        let mut fn_components_fixturator = FnComponentsFixturator::new(::fixt::Unpredictable);

        let zomes: Vec<Zome> = zome_fixturator.take(3).collect();
        let fn_components: FnComponents = fn_components_fixturator.next().unwrap();

        let dna_def = DnaDefBuilder::default()
            .integrity_zomes(
                zomes
                    .clone()
                    .into_iter()
                    .map(|z| (z.name, z.def.into()))
                    .collect(),
            )
            .coordinator_zomes(vec![])
            .modifiers(DnaModifiers {
                network_seed: "".into(),
                properties: SerializedBytes::default(),
            })
            .build()
            .unwrap();
        let dna_def_hashed = DnaDefHashed::from_content_sync(dna_def);

        let mut ribosome_builder = MockRibosomeBuilder::new_with_dna_def(dna_def_hashed);

        invocation
            .expect_zomes()
            .times(1)
            .in_sequence(&mut sequence)
            .return_const(ZomesToInvoke::AllIntegrity);

        invocation
            .expect_fn_components()
            .times(1)
            .in_sequence(&mut sequence)
            .return_const(fn_components.clone());

        invocation
            .expect_auth()
            .return_const(InvocationAuth::LocalCallback);

        // zomes are the outer loop as we process all callbacks in a single zome before moving to
        // the next one
        for zome in zomes.clone() {
            for fn_component in fn_components.clone() {
                // the invocation zome name and component will be called by the ribosome
                ribosome_builder
                    .raw_mock()
                    .expect_maybe_call()
                    .with(
                        always(),
                        always(),
                        always(),
                        function({
                            let zome = zome.clone();
                            move |z: &Zome| z.name == zome.name
                        }),
                        eq(FunctionName::from(fn_component)),
                        always(),
                    )
                    .times(1)
                    .in_sequence(&mut sequence)
                    .returning(|_, _, _, _, _, _| {
                        Box::pin(async move {
                            Ok(Some(ExternIO::encode(InitCallbackResult::Pass).unwrap()))
                        })
                    });
            }
        }

        let ribosome = ribosome_builder.build().await.unwrap();

        let (calls, _h) = call_stream(
            host_access.into(),
            ribosome,
            std::sync::Arc::new(invocation),
        );

        let output: Vec<Result<(_, ExternIO), _>> = calls.collect().await;
        assert!(output.iter().all(|r| r.is_ok()));
        assert_eq!(output.len(), zomes.len() * fn_components.0.len());
    }
}
