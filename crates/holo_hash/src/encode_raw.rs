use crate::HashType;
use crate::HoloHash;

impl<T: HashType, S: HashSerializer> std::fmt::Display for HoloHash<T, S> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        f.write_fmt(format_args!(
            "0x{}",
            holochain_util::hex::bytes_to_hex(self.get_raw_39(), false)
        ))?;
        Ok(())
    }
}
