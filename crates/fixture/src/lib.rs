pub enum FixtureType<I: Sized> {
    A,
    B,
    C,
    Random,
    FromInput(I),
}

pub trait Fixture {
    type Input: Sized;
    fn fixture(fixture_type: FixtureType<Self::Input>) -> Self;
}
