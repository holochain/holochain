use bytes::Bytes;
use rusqlite::{
    types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef},
    ToSql,
};

/// A convenient struct to handle Bytes in SQL queries
pub struct BytesSql(pub Bytes);

impl ToSql for BytesSql {
    #[inline]
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(&self.0[..]))
    }
}

impl FromSql for BytesSql {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(BytesSql(Bytes::copy_from_slice(value.as_blob()?)))
    }
}

/// A shrinkwrapped type with a Drop impl provided as a simple closure
#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct SwanSong<'a, T> {
    #[shrinkwrap(main_field)]
    inner: T,
    #[allow(clippy::type_complexity)]
    song: Option<Box<dyn FnOnce(&mut T) + 'a>>,
}

impl<T> Drop for SwanSong<'_, T> {
    fn drop(&mut self) {
        self.song.take().unwrap()(&mut self.inner);
    }
}

impl<'a, T> SwanSong<'a, T> {
    pub fn new<F: FnOnce(&mut T) + 'a>(inner: T, song: F) -> Self {
        Self {
            inner,
            song: Some(Box::new(song)),
        }
    }
}
