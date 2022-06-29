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
/// A wrapper around [`rusqlite::types::ToSqlOutput`].
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

impl<'a> From<&'a ActionType> for SqlOutput<'a> {
    fn from(d: &'a ActionType) -> Self {
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
            // Filtering on all types is the same as not filtering at all.
            LinkTypeRange::Full => String::new(),
            // Empty ranges return nothing.
            LinkTypeRange::Empty => " false ".to_string(),
            LinkTypeRange::Inclusive(range) => match range.start().0.cmp(&range.end().0) {
                // Start is less than end.
                std::cmp::Ordering::Less => {
                    if range.start().0 == 0 && range.end().0 == u8::MAX {
                        // Range is full.
                        LinkTypeRange::Full.to_sql_statement()
                    } else {
                        // Otherwise it is an inclusive range.
                        // In sql that is `BETWEEN ? AND ?`.
                        format!(
                            " link_type BETWEEN {} AND {} ",
                            range.start().0,
                            range.end().0
                        )
                    }
                }
                // Start is equal to end, so we just match on start.
                std::cmp::Ordering::Equal => format!(" link_type = {} ", range.start().0),
                // Start is greater than end, so this is an empty range.
                std::cmp::Ordering::Greater => LinkTypeRange::Empty.to_sql_statement(),
            },
        }
    }
}

impl ToSqlStatement for LinkTypeRanges {
    fn to_sql_statement(&self) -> String {
        // If any ranges are empty, then we return `AND false`.
        if self.0.iter().any(|r| {
            matches!(r, LinkTypeRange::Empty)
                || matches!(r, LinkTypeRange::Inclusive(inner) if inner.is_empty())
        }) {
            " AND false ".to_string()
        } else if self.0.iter().all(|r| {
            // If all ranges are full, then we return an empty string.
            matches!(r, LinkTypeRange::Full) || matches!(r, LinkTypeRange::Inclusive(r) if r.start().0 == 0 && r.end().0 == u8::MAX)
        }) {
            String::new()
        } else {
            // Collect all the statements.
            let mut out: Vec<String> = self
                .0
                .iter()
                .map(ToSqlStatement::to_sql_statement)
                .filter(|s| !s.is_empty())
                .collect();

            // Remove duplicates.
            out.sort_unstable();
            out.dedup();

            // Interleave with `OR`.
            let mut out = out.into_iter().flat_map(|s| [s, " OR ".to_string()]).collect::<Vec<String>>();

            // Remove the last `OR`.
            out.pop();

            // Wrap the whole filter in `AND ()`.
            format!(" AND ( {} ) ", out.into_iter().collect::<String>())
        }
    }
}
