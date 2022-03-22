//! # Sql Helper types.
//! For some dependencies we don't want to include the rusqlite dependency so
//! we need a way to define the [`rusqlite::ToSql`] trait for types defined
//! in upstream crates.
use holochain_zome_types::prelude::*;
use rusqlite::types::ToSqlOutput;

/// A helper trait for types we can't implement [`rusqlite::ToSql`]
/// for due to the orphan rule.
pub trait AsSql<'a> {
    /// Convert this type to sql which might fail.
    fn as_sql(&'a self) -> SqlOutput<'a>;
}

#[derive(Clone, Debug, PartialEq)]
/// A wrapper around [`rusqlite::ToSqlOutput`].
/// This allows implementing `From<Foo> for SqlOutput`
/// for types defined outside this crate.
pub struct SqlOutput<'a>(pub ToSqlOutput<'a>);

impl<'a> rusqlite::ToSql for SqlOutput<'a> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        rusqlite::ToSql::to_sql(&self.0)
    }
}

impl<'a, T> AsSql<'a> for T
where
    SqlOutput<'a>: From<&'a T>,
    T: 'a,
{
    fn as_sql(&'a self) -> SqlOutput {
        self.into()
    }
}

impl<'a, T> AsSql<'a> for Option<T>
where
    SqlOutput<'a>: From<&'a T>,
    T: 'a,
{
    fn as_sql(&'a self) -> SqlOutput {
        match self {
            Some(d) => d.into(),
            None => SqlOutput(ToSqlOutput::Owned(rusqlite::types::Value::Null)),
        }
    }
}

impl<'a> From<&'a LinkTag> for SqlOutput<'a> {
    fn from(d: &'a LinkTag) -> Self {
        SqlOutput(ToSqlOutput::Borrowed((&d.0[..]).into()))
    }
}

