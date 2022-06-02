//! # Sql Helper types.
//! For some dependencies we don't want to include the rusqlite dependency so
//! we need a way to define the [`rusqlite::ToSql`] trait for types defined
//! in upstream crates.
use holochain_zome_types::prelude::*;
use rusqlite::types::ToSqlOutput;

#[cfg(test)]
mod test;

/// A helper trait for types we can't implement [`rusqlite::ToSql`]
/// for due to the orphan rule.
pub trait AsSql<'a> {
    /// Convert this type to sql which might fail.
    fn as_sql(&'a self) -> SqlOutput<'a>;
}

/// A trait to convert a reference of a
/// type to a sql statement for use in
/// [`rusqlite::Connection`] prepare.
pub trait ToSqlStatement {
    /// Convert the reference to a statement.
    fn to_sql_statement(&self) -> String;
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

fn via_display(data: &impl std::fmt::Display) -> SqlOutput {
    SqlOutput(ToSqlOutput::Owned(data.to_string().into()))
}

impl<'a> From<&'a HeaderType> for SqlOutput<'a> {
    fn from(d: &'a HeaderType) -> Self {
        via_display(d)
    }
}

impl<'a> From<&'a EntryType> for SqlOutput<'a> {
    fn from(d: &'a EntryType) -> Self {
        via_display(d)
    }
}

impl<'a> From<&'a LinkTag> for SqlOutput<'a> {
    fn from(d: &'a LinkTag) -> Self {
        SqlOutput(ToSqlOutput::Borrowed((&d.0[..]).into()))
    }
}

impl<'a, 'b> From<&'b ZomeId> for SqlOutput<'a> {
    fn from(d: &'b ZomeId) -> Self {
        Self(d.0.into())
    }
}

impl ToSqlStatement for LinkTypeRange {
    fn to_sql_statement(&self) -> String {
        match self {
            LinkTypeRange::Full => String::new(),
            LinkTypeRange::Empty => " AND false ".to_string(),
            LinkTypeRange::Inclusive(range) => match range.start().0.cmp(&range.end().0) {
                std::cmp::Ordering::Less => {
                    if range.start().0 == 0 && range.end().0 == u8::MAX {
                        LinkTypeRange::Full.to_sql_statement()
                    } else {
                        format!(
                            " AND link_type BETWEEN {} AND {} ",
                            range.start().0,
                            range.end().0
                        )
                    }
                }
                std::cmp::Ordering::Equal => format!(" AND link_type = {} ", range.start().0),
                std::cmp::Ordering::Greater => LinkTypeRange::Empty.to_sql_statement(),
            },
        }
    }
}

impl ToSqlStatement for LinkTypeRanges {
    fn to_sql_statement(&self) -> String {
        if self.0.iter().any(|r| {
            matches!(r, LinkTypeRange::Empty)
                || matches!(r, LinkTypeRange::Inclusive(inner) if inner.is_empty())
        }) {
            " AND false ".to_string()
        } else {
            let mut out: Vec<String> = self
                .0
                .iter()
                .map(ToSqlStatement::to_sql_statement)
                .collect();
            out.sort_unstable();
            out.dedup();
            out.into_iter().collect()
        }
    }
}
