mod cli;
mod error;
mod init;
mod packing;

pub use cli::{
    app_pack_recursive, bundled_dnas_workdir_locations, get_app_name, get_dna_name,
    get_web_app_name, web_app_pack_recursive, HcAppBundle, HcDnaBundle, HcWebAppBundle,
};
pub use packing::{pack, unpack, unpack_raw};

// Temporarily include a fork of wasmer from the git branch 'wamr', until it is officially released in wasmer v5
#[cfg(feature = "wasmer_wamr")]
extern crate hc_wasmer as wasmer;