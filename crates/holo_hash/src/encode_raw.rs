use crate::HashType;
use crate::HoloHash;

impl<T: HashType> std::fmt::Display for HoloHash<T> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        f.write_fmt(format_args!("0x"))?;
        for byte in self.get_raw_39() {
            f.write_fmt(format_args!("{:02x}", byte))?;
        }
        Ok(())
    }
}
