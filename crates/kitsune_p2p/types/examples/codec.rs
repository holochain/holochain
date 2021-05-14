use kitsune_p2p_types::codec::*;
use kitsune_p2p_types::*;

write_codec_enum! {
    /// My codec is awesome.
    codec MyCodec {
        /// My codec has only one variant.
        MyVariant(0x00) {
            /// My variant has only one type
            my_type.0: String,
        },
    }
}

fn main() {
    let item1 = MyCodec::my_variant("test".to_string());
    let data = item1.encode_vec().unwrap();
    println!("Encoded: {:?}", &data);
    let (_, item2) = MyCodec::decode_ref(&data).unwrap();
    println!("Decoded: {:?}", item2);
    assert_eq!(item1, item2);
}
