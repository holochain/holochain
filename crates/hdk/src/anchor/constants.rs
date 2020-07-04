/// the string "anchor" as utf8 bytes
pub const ANCHOR: [u8; 6] = [0x61, 0x6e, 0x63, 0x68, 0x6f, 0x72];

#[test]
#[cfg(test)]
fn anchor_anchor() {
    assert_eq!("anchor".as_bytes(), ANCHOR,);
}
