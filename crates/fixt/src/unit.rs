use crate::prelude::*;

/// it doesn't matter what curve you pass in, we will send () back
impl<Curve> Iterator for Fixturator<(), Curve> {
    type Item = ();

    fn next(&mut self) -> Option<Self::Item> {
        Some(())
    }
}

impl Fixt for () {}

#[cfg(test)]
pub mod tests {
    use crate::prelude::*;

    type Unit = ();

    basic_test!(Unit, (), vec![(), (), (), (), ()], false);
}
