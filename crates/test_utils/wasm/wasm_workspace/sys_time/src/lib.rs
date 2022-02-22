use hdk::prelude::*;

#[hdk_extern]
fn sys_time(_: ()) -> ExternResult<Timestamp> {
    hdk::prelude::sys_time()
}

#[cfg(test)]
pub mod test {
    use hdk::prelude::*;

    #[test]
    fn sys_time_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        mock_hdk
            .expect_sys_time()
            .with(hdk::prelude::mockall::predicate::eq(()))
            .times(1)
            .return_once(|_| Ok(Timestamp::from_micros(5)));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::sys_time(());

        assert_eq!(result, Ok(Timestamp::from_micros(5)))
    }
}
