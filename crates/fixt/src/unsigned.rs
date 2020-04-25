use rand::seq::SliceRandom;

impl Iterator for Fixturator<Unpredictable, u32> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        // we want a high probability to output an impolite value
        // impolite values are those that common edge cases that lead to bugs
        let polite: bool = rand::random();
        if polite {
            Some(rand::random())
        } else {
            let impolite_vals = vec![u32::max_value(), u32::min_value(), 1];
            impolite_vals.choose(&mut rand::thread_rng()).cloned()
        }
    }
}

impl Iterator for Fixturator<Predictable, u32> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        self.index = self.index + 1;
        Some(self.index as _)
    }
}

impl Iterator for Fixturator<Empty, u32> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        Some(0)
    }
}

impl Fixt for u32 {}




#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    pub fn u32_test () {
        let mut fixturator = u32::fixturator::<Empty>();
        println!("{:?}", &mut fixturator.next());
        assert_eq!(0, fixturator.next().unwrap());

        let mut unpredictable_fixturator = u32::fixturator::<Unpredictable>();
        println!("{:?}", &mut unpredictable_fixturator.next());
    }
}
