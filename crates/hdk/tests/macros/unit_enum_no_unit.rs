use hdk::prelude::hdi::prelude::*;

#[derive(UnitEnum)]
enum Nesting {
    A(A),
    B(A),
}

struct A;

fn main() {}
