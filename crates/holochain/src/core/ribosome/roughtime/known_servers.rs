// @TODO
// https://roughtime.googlesource.com/roughtime/+/HEAD/ECOSYSTEM.md#curating-server-lists
// So, instead, Roughtime is only available for products that can be updated. The server lists have
// an explicit expiry time in them and we will actively seek to break clients that try to use old
// information in order to maintain ecosystem health. At the moment changing the hostname or port
// of a server is the easiest way to enforce this but we expect to add a per-server id in the
// future that clients would need to send in order to prove to the server that they have a current
// server list.
// https://github.com/cloudflare/roughtime/blob/master/ecosystem.json
const CLOUDFLARE_ADDR: &str = "roughtime.cloudflare.com:2002";
const CLOUDFLARE_PUB_KEY: &[u8; 32] = &[
    0x80, 0x3e, 0xb7, 0x85, 0x28, 0xf7, 0x49, 0xc4, 0xbe, 0xc2, 0xe3, 0x9e, 0x1a, 0xbb, 0x9b, 0x5e,
    0x5a, 0xb7, 0xe4, 0xdd, 0x5c, 0xe4, 0xb6, 0xf2, 0xfd, 0x2f, 0x93, 0xec, 0xc3, 0x53, 0x8f, 0x1a,
];

const CAESIUM_ADDR: &str = "caesium.tannerryan.ca:2002";
const CAESIUM_PUB_KEY: &[u8; 32] = &[
    0x88, 0x15, 0x63, 0xc6, 0x0f, 0xf5, 0x8f, 0xbc, 0xb5, 0xfa, 0x44, 0x14, 0x4c, 0x16, 0x1d, 0x4d,
    0xa6, 0xf1, 0x0a, 0x9a, 0x5e, 0xb1, 0x4f, 0xf4, 0xec, 0x3e, 0x0f, 0x30, 0x32, 0x64, 0xd9, 0x60,
];

const TICKTOCK_ADDR: &str = "ticktock.mixmin.net:5333";
const TICKTOCK_PUB_KEY: &[u8; 32] = &[
    0x72, 0x3f, 0x06, 0xb2, 0x23, 0x65, 0x46, 0x4a, 0xa2, 0x0c, 0x49, 0x40, 0x78, 0xd3, 0x12, 0x04,
    0x13, 0x30, 0xac, 0x09, 0x75, 0xe6, 0x16, 0x0f, 0x81, 0x7e, 0x74, 0xf8, 0x65, 0x97, 0xfe, 0x50,
];

pub struct Server {
    addr: String,
    pub_key: [u8; 32],
}

impl Server {
    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub fn pub_key(&self) -> &[u8; 32] {
        &self.pub_key
    }
}

pub fn servers() -> Vec<Server> {
    vec![
        Server {
            addr: CLOUDFLARE_ADDR.into(),
            pub_key: *CLOUDFLARE_PUB_KEY,
        },
        Server {
            addr: CAESIUM_ADDR.into(),
            pub_key: *CAESIUM_PUB_KEY,
        },
        Server {
            addr: TICKTOCK_ADDR.into(),
            pub_key: *TICKTOCK_PUB_KEY,
        },
    ]
}
