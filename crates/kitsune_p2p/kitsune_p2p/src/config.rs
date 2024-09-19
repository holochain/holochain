use kitsune_p2p_types::config::{tuning_params_struct, KitsuneP2pTuningParams};
use kitsune_p2p_types::tx2::tx2_adapter::AdapterFactory;
use kitsune_p2p_types::tx_utils::*;
use kitsune_p2p_types::*;
use url2::Url2;

// TODO - FIXME - holochain bootstrap should not be encoded in kitsune
/// The default production bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEFAULT: &str = "https://bootstrap-staging.holo.host";

// TODO - FIXME - holochain bootstrap should not be encoded in kitsune
/// The default development bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEV: &str = "https://bootstrap-dev.holohost.workers.dev";
