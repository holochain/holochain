use hdk::prelude::*;

#[hdk_extern]
fn sys_time(_: ()) -> ExternResult<core::time::Duration> {
    hdk::prelude::sys_time()
}

#[cfg(test)]
pub mod test {

    #[test]
    fn sys_time_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        mock_hdk.expect_sys_time()
            .with(hdk::prelude::mockall::predicate::eq(()))
            .times(1)
            .return_once(|_| Ok(core::time::Duration::new(5, 0)));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::sys_time(());

        assert_eq!(
            result,
            Ok(
                core::time::Duration::new(5, 0)
            )
        )
    }
}