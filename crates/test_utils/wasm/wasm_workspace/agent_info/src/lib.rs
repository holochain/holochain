use hdk::prelude::*;

#[hdk_entry(id = "thing")]
struct Thing;

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn call_info(_: ()) -> ExternResult<CallInfo> {
    hdk::prelude::call_info()
}

#[hdk_extern]
fn agent_info(_: ()) -> ExternResult<AgentInfo> {
    hdk::prelude::create_entry(Thing)?;
    hdk::prelude::agent_info()
}

#[cfg(test)]
pub mod test {
    use hdk::prelude::*;
    use ::fixt::prelude::*;

    #[test]
    fn agent_info_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let agent_info = fixt!(AgentInfo);
        let closure_agent_info = agent_info.clone();
        mock_hdk.expect_create()
            .times(1)
            .return_once(move |_| Ok(fixt!(HeaderHash)));
        mock_hdk.expect_agent_info()
            .with(hdk::prelude::mockall::predicate::eq(()))
            .times(1)
            .return_once(move |_| Ok(closure_agent_info));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::agent_info(());

        assert_eq!(
            result,
            Ok(
                agent_info
            )
        )
    }
}