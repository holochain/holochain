use hdk_derive::*;

#[derive(EntryDefRegistration)]
enum Nesting {
    #[entry_type(name = 55)]
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
