//! Combinator functions, for more easeful functional programming

/// Return the first element of a 2-tuple
pub fn first<A, B>(tup: (A, B)) -> A {
    tup.0
}

/// Return the second element of a 2-tuple
pub fn second<A, B>(tup: (A, B)) -> B {
    tup.1
}
