pub enum FixtureType {
    A,
    B,
    C,
    Random,
}

pub trait Fixture {
    fn fixture(fixture_type: FixtureType) -> Self;
}
