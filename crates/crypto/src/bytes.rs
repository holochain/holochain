use crate::*;

mod safe_mem_copy;

/// read guard for crypto bytes
pub trait CryptoBytesRead<'lt>: 'lt + std::ops::Deref<Target = [u8]> {}

/// dyn reference to read guard
pub type DynCryptoBytesRead<'lt> = Box<dyn CryptoBytesRead<'lt> + 'lt>;

/// write guard for crypto bytes
pub trait CryptoBytesWrite<'lt>: 'lt + CryptoBytesRead<'lt> + std::ops::DerefMut {}

/// dyn reference to write guard
pub type DynCryptoBytesWrite<'lt> = Box<dyn CryptoBytesWrite<'lt> + 'lt>;

/// crypto bytes are used for communicating with sodium functions
pub trait CryptoBytes: 'static + Send + std::fmt::Debug {
    /// clone these bytes
    fn clone(&self) -> DynCryptoBytes;

    /// size of this byte buffer
    fn len(&self) -> usize;

    /// is this a zero length buffer?
    fn is_empty(&self) -> bool;

    /// get a read guard for this byte buffer
    fn read(&self) -> DynCryptoBytesRead;

    /// get a write guard for this byte buffer
    fn write(&mut self) -> DynCryptoBytesWrite;

    /// copy data from another byte array into this buffer
    fn copy_from(&mut self, offset: usize, data: &[u8]) -> CryptoResult<()> {
        safe_mem_copy::safe_mem_copy(&mut self.write(), offset, data)
    }
}

/// dyn reference to crypto bytes
pub type DynCryptoBytes = Box<dyn CryptoBytes + 'static + Send>;

impl Clone for DynCryptoBytes {
    fn clone(&self) -> Self {
        CryptoBytes::clone(&**self)
    }
}

/// internal insecure buffer
#[derive(Debug)]
pub(crate) struct InsecureBytes(Vec<u8>);

impl InsecureBytes {
    /// internal constructor
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new(size: usize) -> DynCryptoBytes {
        Box::new(Self(vec![0; size]))
    }
}

impl<'lt> CryptoBytesRead<'lt> for &'lt [u8] {}
impl<'lt> CryptoBytesRead<'lt> for &'lt mut [u8] {}
impl<'lt> CryptoBytesWrite<'lt> for &'lt mut [u8] {}

impl CryptoBytes for InsecureBytes {
    fn clone(&self) -> DynCryptoBytes {
        Box::new(InsecureBytes(self.0.clone()))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn read(&self) -> DynCryptoBytesRead {
        let r: &[u8] = &self.0;
        Box::new(r)
    }

    fn write(&mut self) -> DynCryptoBytesWrite {
        let r: &mut [u8] = &mut self.0;
        Box::new(r)
    }
}
