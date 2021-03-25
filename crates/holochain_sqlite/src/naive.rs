mod table;
use rusqlite::ToSql;
pub use table::*;

pub struct PartialStatement<'a> {
    pub statement: &'static str,
    pub params: &'a [(&'static str, &'a dyn ToSql)],
}
