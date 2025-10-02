//! Combinator functions, for more easeful functional programming

/// Map an iterator into a vec of a new type
pub fn mapvec<'a, T: 'a, U>(it: impl Iterator<Item = &'a T>, f: impl FnMut(&'a T) -> U) -> Vec<U> {
    it.map(f).collect()
}
