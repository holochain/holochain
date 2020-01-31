use sx_types::prelude::*;

#[derive(PartialEq, Eq, PartialOrd, Hash, Clone, serde::Serialize, Debug)]
pub enum Chain {
    Todo,
}
impl Attribute for Chain {}
