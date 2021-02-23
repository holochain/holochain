use rkv::{MultiIntegerStore, PrimitiveInt, StoreError, StoreOptions};

use crate::exports::*;

/// SHIM
pub struct Rkv {}

impl Rkv {
    /// SHIM
    pub fn open_single<'s, T>(&self, name: T, opts: StoreOptions) -> Result<SingleStore, StoreError>
    where
        T: Into<Option<&'s str>>,
    {
        todo!("this is a shim")
    }

    /// SHIM
    pub fn open_integer<'s, T>(
        &self,
        name: T,
        mut opts: StoreOptions,
    ) -> Result<IntegerStore, StoreError>
    where
        T: Into<Option<&'s str>>,
    {
        todo!("this is a shim")
    }

    /// SHIM
    pub fn open_multi<'s, T>(
        &self,
        name: T,
        mut opts: StoreOptions,
    ) -> Result<MultiStore, StoreError>
    where
        T: Into<Option<&'s str>>,
    {
        todo!("this is a shim")
    }

    /// SHIM
    pub fn open_multi_integer<'s, T, K: PrimitiveInt>(
        &self,
        name: T,
        mut opts: StoreOptions,
    ) -> Result<MultiIntegerStore<K>, StoreError>
    where
        T: Into<Option<&'s str>>,
    {
        todo!("this is a shim")
    }
}
