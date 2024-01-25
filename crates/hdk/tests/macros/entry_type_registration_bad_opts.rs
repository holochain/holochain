use hdk::prelude::hdi::prelude::*;

#[derive(EntryDefRegistration)]
enum Nesting {
    #[entry_type(nam = "a")]
    A(A),
    B(B),
}

enum A {
    A,
    B,
}

enum B {
    A,
    B,
}

fn main() {}
