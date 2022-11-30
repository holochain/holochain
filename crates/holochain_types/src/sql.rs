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

impl<'a, 'b> From<&'b ZomeIndex> for SqlOutput<'a> {
    fn from(d: &'b ZomeIndex) -> Self {
        Self(d.0.into())
    }
}

impl ToSqlStatement for LinkTypeFilter {
    fn to_sql_statement(&self) -> String {
        match self {
            LinkTypeFilter::Types(types) => {
                match types
                    .first()
                    .filter(|(_, t)| types.len() == 1 && t.len() == 1)
                    .and_then(|(z, t)| t.first().map(|t| (z, t)))
                {
                    Some((zome_index, link_type)) => {
                        format!(
                            " AND zome_index = {} AND link_type = {} ",
                            zome_index.0, link_type.0
                        )
                    }
                    _ => {
                        let mut out = types
                            .iter()
                            .flat_map(|(zome_index, types)| {
                                let mut types: Vec<String> = types
                                    .iter()
                                    .flat_map(|t| {
                                        [format!(" link_type = {} ", t.0), "OR".to_string()]
                                    })
                                    .collect();

                                // Pop last " OR "
                                types.pop();

                                [
                                    format!(
                                        " ( zome_index = {} AND ({}) ) ",
                                        zome_index.0,
                                        types.into_iter().collect::<String>()
                                    ),
                                    "OR".to_string(),
                                ]
                            })
                            .collect::<Vec<_>>();
                        // Pop last " OR "
                        out.pop();
                        if out.is_empty() {
                            String::new()
                        } else {
                            format!(" AND ({}) ", out.into_iter().collect::<String>())
                        }
                    }
                }
            }
            LinkTypeFilter::Dependencies(dependencies) => {
                let mut out = dependencies
                    .iter()
                    .flat_map(|z| [format!(" zome_index = {} ", z.0), "OR".to_string()])
                    .collect::<Vec<_>>();
                // Pop last " OR "
                out.pop();
                if out.is_empty() {
                    String::new()
                } else {
                    format!(" AND ({}) ", out.into_iter().collect::<String>())
                }
            }
        }
    }
}
