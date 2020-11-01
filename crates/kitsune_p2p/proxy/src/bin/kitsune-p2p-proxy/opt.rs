/// Option Parsing
#[derive(structopt::StructOpt, Debug)]
#[structopt(name = "kitsune-p2p-proxy")]
pub struct Opt {
    /// To which network interface / port should we bind?
    /// Default: "kitsune-quic://0.0.0.0:0".
    #[structopt(short = "b", long)]
    pub bind_to: Option<String>,

    /// If you have port-forwarding set up,
    /// or wish to apply a vanity domain name,
    /// you may need to override the local NIC ip.
    /// Default: None = use NIC ip.
    #[structopt(short = "h", long)]
    pub override_host: Option<String>,
}

impl From<&Opt> for kitsune_p2p_transport_quic::ConfigListenerQuic {
    fn from(o: &Opt) -> Self {
        let mut out = Self::default();
        if let Some(b) = &o.bind_to {
            out = out.set_bind_to(Some(kitsune_p2p_types::dependencies::url2::url2!("{}", b)));
        }
        if let Some(h) = &o.override_host {
            out = out.set_override_host(Some(h));
        }
        out
    }
}

impl From<Opt> for kitsune_p2p_transport_quic::ConfigListenerQuic {
    fn from(o: Opt) -> Self {
        (&o).into()
    }
}
