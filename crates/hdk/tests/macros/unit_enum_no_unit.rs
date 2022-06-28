use hdk::prelude::holochain_deterministic_integrity::prelude::*;

#[derive(UnitEnum)]
enum Nesting {
    A(A),
    B(A),
}

struct A;

fn main() {}
