/// get the entry hash for anything that that implements TryInto<SerializedBytes>
///
/// anything that is annotated with #[hdk_entry( .. )] or entry_def!( .. ) implements this so is
/// compatible automatically.
///
/// entry_hash! is "dumb" in that it doesn't check the entry is defined, committed, on the DHT or
/// any other validation, it simply generates the hash for the serialized representation of
/// something in the same way that the DHT would.
///
/// it is strongly recommended that you use the entry_hash host_fn to calculate hashes to avoid
/// inconsistencies between hashes in the wasm guest and the host.
/// for example, a lot of the crypto crates in rust compile to wasm so in theory could generate the
/// hash in the guest, but there is the potential that the serialization logic could be slightly
/// different, etc.
///
/// ```ignore
/// let foo_hash = entry_hash!(foo)?;
/// ```
///
/// the hashes produced by entry_hash are directly compatible with other macros that accept an
/// entry hash, for example `get!(entry_hash!(foo)?)?` would attempt to get a copy of `foo` from
/// the DHT.
#[macro_export]
macro_rules! hash_entry {
    ( $input:expr ) => {{
        $crate::prelude::host_externs!(__hash_entry);

        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::host_fn!(
                __hash_entry,
                $crate::prelude::HashEntryInput::new($crate::prelude::Entry::App(sb.try_into()?)),
                $crate::prelude::HashEntryOutput
            ),
            Err(e) => Err(e),
        }
    }};
}
