use hdk::prelude::hdi::prelude::*;

#[derive(EntryDefRegistration)]
enum Nesting {
    #[entry_def(visibility = "open")]
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
