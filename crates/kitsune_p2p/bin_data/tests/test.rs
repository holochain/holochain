use kitsune_p2p_bin_data::{KitsuneBinType, KitsuneOpData, KitsuneSpace, NodeCert};

#[test]
fn create_bin_type() {
    let input_bytes = vec![5; 32];
    let space = KitsuneSpace::new(input_bytes.clone());

    assert_eq!(32, space.get_bytes().len());
    assert_eq!(&input_bytes, space.get_bytes());
    assert_eq!(0, space.get_loc().as_u32()); // default location bytes
}

#[test]
fn debug_format_space() {
    let input_bytes = vec![5; 32];
    let space = KitsuneSpace::new(input_bytes.clone());

    assert_eq!(
        "KitsuneSpace(0x050505050505050505050505050505050505050505050505050505050505050500000000)",
        format!("{:?}", space)
    );
}

#[test]
fn debug_format_op_data() {
    let input_bytes = vec![5; 32];
    let op_data = KitsuneOpData::new(input_bytes.clone());

    assert_eq!(
        "KitsuneOpData(0x0505050505050505050505050505050505050505050505050505050505050505)",
        format!("{:?}", op_data)
    );
}

#[test]
fn debug_format_op_data_long() {
    let input_bytes = vec![5; 128];
    let op_data = KitsuneOpData::new(input_bytes.clone());

    assert_eq!(
        "KitsuneOpData(0x0505050505050505..0505050505050505; len=16)",
        format!("{:?}", op_data)
    );
}

#[test]
fn debug_format_node_cert() {
    let input_bytes = vec![5; 32];
    let node_cert = NodeCert::from(std::sync::Arc::new(input_bytes.try_into().unwrap()));

    assert_eq!(
        "NodeCert(0x0505050505050505050505050505050505050505050505050505050505050505)",
        format!("{:?}", node_cert)
    );
}
