use crate::prelude::*;

impl Iterator for Fixturator<bool, Empty> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        Some(false)
    }
}

impl Iterator for Fixturator<bool, Unpredictable> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        Some(rand::random())
    }
}

impl Iterator for Fixturator<bool, Predictable> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        self.index = self.index + 1;
        Some(if self.index % 2 == 0 { false } else { true })
    }
}

impl Fixt for bool {}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    basic_test!(
        bool,
        false,
        10,
        vec![true, false, true, false, true, false, true, false, true, false]
    );
}
