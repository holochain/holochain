use crate::{HashType, HoloHashImpl};

impl<T: HashType> std::fmt::Display for HoloHashImpl<T> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        f.write_fmt(format_args!("0x"))?;
        for byte in self.get_raw() {
            f.write_fmt(format_args!("{:02x}", byte))?;
        }
        Ok(())
    }
}
