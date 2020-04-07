impl Fixture for HashString {
    fn fixture(fixture_type: FixtureType) -> Self {
        match fixture_type {
            FixtureType::A => {
                SerializedBytes::try_from(test_entry_a()).unwrap().address()
            }
        }
    }
}
