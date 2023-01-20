//! Combinator functions, for more easeful functional programming

/// Return the first element of a 2-tuple
pub fn first<A, B>(tup: (A, B)) -> A {
    tup.0
}

/// Return the first element of a 2-tuple ref
pub fn first_ref<A, B>(tup: &(A, B)) -> &A {
    &tup.0
}

/// Return the second element of a 2-tuple
pub fn second<A, B>(tup: (A, B)) -> B {
    tup.1
}

/// Return the second element of a 2-tuple ref
pub fn second_ref<A, B>(tup: &(A, B)) -> &B {
    &tup.1
}

/// Swap the two items in 2-tuple
pub fn swap2<A, B>(tup: (A, B)) -> (B, A) {
    (tup.1, tup.0)
}
