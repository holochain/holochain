use hdk::prelude::*;

const TICKS: usize = 5;

#[hdk_entry(id = "tick")]
struct Tick;

#[hdk_extern(infallible)]
fn scheduled_fn(prev_schedule: Option<Schedule>) -> Option<Schedule> {
    if create(Tick).is_err() {
        return Some(Scheduel::Ephemeral(std::time::Duration::from_millis(1)));
    }
    if query(ChainQueryFilter::default().entry_type(entry_type!(Tick))).unwrap().len() < TICKS {
        Some(Schedule::Ephemeral(std::time::Duration::from_millis(1)))
    }
    else {
        None
    }
}

#[hdk_extern]
fn schedule(bytes: u32) -> ExternResult<Bytes> {
    hdk::prelude::schedule("scheduled_fn")
}

#[cfg(test)]
pub mod tests {
    #[test]
    fn random_bytes_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input = 1;
        let output = hdk::prelude::Bytes::from(vec![4_u8]);
        let output_closure = output.clone();
        mock_hdk.expect_random_bytes()
            .with(hdk::prelude::mockall::predicate::eq(
                input
            ))
            .times(1)
            .return_once(move |_| Ok(output_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::random_bytes(input);

        assert_eq!(
            result,
            Ok(
                output
            )
        );
    }
}